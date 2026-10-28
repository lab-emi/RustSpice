//! Accuracy metrics (SUITE_SPEC section 4) and the status/outcome classification (section 5).

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Duration;

use serde::Serialize;

use super::meta::{find_tran_tokens, Expect, Meta, TranTokens};
use super::oracle;
use super::rawfile::{Plot, PlotKind};

// ---------------------------------------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    FailAccuracy,
    FailRobustness,
    FailSilent,
    KnownGapConfirmed,
    GapClosed,
    BothError,
    NgspiceError,
    Panic,
    Timeout,
    SkippedBudget,
    Unverified,
    Info,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::FailAccuracy => "fail_accuracy",
            Status::FailRobustness => "fail_robustness",
            Status::FailSilent => "fail_silent",
            Status::KnownGapConfirmed => "known_gap_confirmed",
            Status::GapClosed => "gap_closed",
            Status::BothError => "both_error",
            Status::NgspiceError => "ngspice_error",
            Status::Panic => "panic",
            Status::Timeout => "timeout",
            Status::SkippedBudget => "skipped_budget",
            Status::Unverified => "unverified",
            Status::Info => "info",
        }
    }
    /// Statuses that make the run verdict `FAIL`.
    pub fn is_fail(self) -> bool {
        matches!(
            self,
            Status::FailAccuracy
                | Status::FailRobustness
                | Status::FailSilent
                | Status::Panic
                | Status::Timeout
        )
    }
    pub const ALL: [Status; 13] = [
        Status::Pass,
        Status::FailAccuracy,
        Status::FailRobustness,
        Status::FailSilent,
        Status::KnownGapConfirmed,
        Status::GapClosed,
        Status::BothError,
        Status::NgspiceError,
        Status::Panic,
        Status::Timeout,
        Status::SkippedBudget,
        Status::Unverified,
        Status::Info,
    ];
}

/// What happened, independent of what was expected (drives the capability matrix).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Match,
    Mismatch,
    LoudError,
    SilentNumbers,
    Panic,
    Timeout,
    NgspiceOnlyError,
    BothError,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Match => "match",
            Outcome::Mismatch => "mismatch",
            Outcome::LoudError => "loud_error",
            Outcome::SilentNumbers => "silent_numbers",
            Outcome::Panic => "panic",
            Outcome::Timeout => "timeout",
            Outcome::NgspiceOnlyError => "ngspice_only_error",
            Outcome::BothError => "both_error",
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Metric results
// ---------------------------------------------------------------------------------------------------------

/// One point of a per-point error listing (evidence, never used for a verdict).
#[derive(Clone, Debug, Serialize)]
pub struct PointError {
    /// Sweep value.
    pub at: f64,
    pub rustspice: f64,
    pub reference: f64,
    pub abs_err: f64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ProbeResult {
    pub name: String,
    /// Which analysis the probe belongs to (`op`, `dc`, `tran`).
    pub analysis: String,
    /// max |rustspice - reference| over the sweep/time axis (probe units).
    pub abs_err: Option<f64>,
    /// max |rustspice - reference| / max(|reference|, floor).
    pub rel_err: Option<f64>,
    /// Transient only: abs_err / max(swing(reference), floor).
    pub norm_err: Option<f64>,
    pub rms_norm_err: Option<f64>,
    /// Sweep value / time of the largest absolute error (null for `.op`).
    pub worst_at: Option<f64>,
    pub vacuous: bool,
    /// `Some(true|false)` when evaluated; `None` when vacuous, missing in the reference, or not comparable.
    pub pass: Option<bool>,
    /// Same metric for the as-is ngspice run versus the reference (abs for op/dc, norm for tran).
    pub ng_err: Option<f64>,
    /// Rustspice error measure in the same unit as `ng_err` (abs for op/dc, norm for tran).
    pub rs_err: Option<f64>,
    /// `rs_err <= max(ng_err, tolerance)`.
    pub within_ng_error: Option<bool>,
    pub oracle_rs_err: Option<f64>,
    pub oracle_ng_err: Option<f64>,
    /// Maximum 50 %-crossing time shift (tag `timing_edge`), each waveform's crossings located by linear interpolation
    /// between its own rows: what a reader of the two rawfiles sees.
    pub edge_shift: Option<f64>,
    /// The same shift with the reference read at RustSpice's own rows (same sampling, same interpolation): the
    /// simulator's own timing error, without the resolution of its output rows.
    #[serde(default)]
    pub edge_shift_at_rows: Option<f64>,
    /// Maximum shift of the reference's own crossings when it is read only at RustSpice's rows: the resolution limit
    /// of those rows (an edge shorter than the row spacing cannot be placed by linear interpolation).
    #[serde(default)]
    pub edge_row_floor: Option<f64>,
    /// `error / allowed error` for the probe's own metric: at most 1 passes. `None` for vacuous probes.
    pub tol_ratio: Option<f64>,
    /// Transient only: linear-resampling noise of the reference for this probe (normalised), see
    /// [`decimation_floor`].
    pub interp_floor: Option<f64>,
    pub note: Option<String>,
    /// Transient only, EVIDENCE (never used for pass/fail): the error of RustSpice evaluated at its OWN time rows
    /// (the reference interpolated to them), normalised like `norm_err`. A simulator that is exact at its own rows but
    /// whose rows are shifted off the nominal `k * tstep` grid (a Newton retry moves every later row) is charged the
    /// linear-resampling error of its own waveform by the nominal-grid comparison; this field shows how much of
    /// `norm_err` is that.
    pub own_grid_err: Option<f64>,
    /// Same at RustSpice's own rows against the oracle of the case (when it has one and it could be evaluated).
    pub oracle_own_grid_err: Option<f64>,
    /// dc sweep whose grids do not match (point count or an axis value): number of leading points on which the two
    /// sweeps are aligned and were compared as EVIDENCE (the structural failure decides the verdict, not these).
    pub aligned_points: Option<usize>,
    /// Max |rustspice - reference| over the aligned points.
    pub aligned_abs_err: Option<f64>,
    /// Sweep value of that maximum.
    pub aligned_worst_at: Option<f64>,
    /// Per-point errors over the aligned points (at most 64: evenly thinned, always including the first, the worst and
    /// the last aligned point).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub point_errors: Vec<PointError>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OracleSummary {
    pub spec: String,
    /// `ok` | `unsupported` | `panicked` | `budget` (the oracle ran out of its wall-clock / work budget and was cut off:
    /// it did not fail on the deck, so there is no oracle value, not even a "no")
    pub status: String,
    pub reason: Option<String>,
    pub rs_max_err: Option<f64>,
    pub ng_max_err: Option<f64>,
    /// `abs` (op/dc) or `norm` (tran)
    pub metric: String,
    /// Largest forward-error bound (`cond_1 * 1.1e-16`, relative) of the oracle's dense linear solves of this case, see
    /// [`oracle::take_forward_error_bound`]; `None` when the oracle solved no linear system (null, not 0).
    pub fwd_bound: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CompareResult {
    pub analysis: String,
    pub probes: Vec<ProbeResult>,
    /// `Some(true)`: every evaluated probe is within tolerance (and at least one was evaluated);
    /// `Some(false)`: a violation; `None`: not verifiable.
    pub pass: Option<bool>,
    pub failures: Vec<String>,
    pub notes: Vec<String>,
    pub points_rs: Option<usize>,
    pub points_ref: Option<usize>,
    /// Transient: linear-resampling error of the reference itself (normalised), worst probe.
    pub interp_floor: Option<f64>,
    pub oracle: Option<OracleSummary>,
    /// Headline scalar errors (max over evaluated probes; norm for tran, abs otherwise): rustspice / as-is ngspice.
    pub err_rs: Option<f64>,
    pub err_ng: Option<f64>,
    pub within_ng_error: Option<bool>,
    /// Every probe was vacuous (reference ~0) and none deviates: nothing could be evaluated, nothing is wrong.
    pub vacuous_only: bool,
}

pub struct CompareInput<'a> {
    pub meta: &'a Meta,
    pub deck: &'a str,
    pub rs: &'a [Plot],
    pub reference: &'a [Plot],
    /// As-is ngspice plots when they differ from the reference.
    pub asis: Option<&'a [Plot]>,
}

/// Analysis kinds present in the deck (cards after the title line), in deck order.
pub fn detect_kinds(deck: &str) -> Vec<PlotKind> {
    let mut v = Vec::new();
    for (i, line) in deck.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let t = line.trim_start().to_ascii_lowercase();
        let card = |c: &str| {
            t.starts_with(c)
                && t[c.len()..]
                    .chars()
                    .next()
                    .is_none_or(|x| x.is_whitespace())
        };
        let k = if card(".op") {
            Some(PlotKind::Op)
        } else if card(".dc") {
            Some(PlotKind::Dc)
        } else if card(".tran") {
            Some(PlotKind::Tran)
        } else {
            None
        };
        if let Some(k) = k {
            if !v.contains(&k) {
                v.push(k);
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------------------
// Numerical helpers
// ---------------------------------------------------------------------------------------------------------

/// Linear interpolation on a non-decreasing time axis; clamps outside the range.
pub fn interp(ts: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = ts.len().min(ys.len());
    if n == 0 {
        return f64::NAN;
    }
    let i = ts[..n].partition_point(|t| *t <= x);
    if i == 0 {
        return ys[0];
    }
    if i >= n {
        return ys[n - 1];
    }
    let (t0, t1) = (ts[i - 1], ts[i]);
    if t1 <= t0 {
        return ys[i];
    }
    ys[i - 1] + (ys[i] - ys[i - 1]) * (x - t0) / (t1 - t0)
}

/// `true` when [`interp`] at `x` is computed from finite samples only.
fn finite_at(ts: &[f64], ys: &[f64], x: f64) -> bool {
    let n = ts.len().min(ys.len());
    if n == 0 {
        return false;
    }
    let i = ts[..n].partition_point(|t| *t <= x);
    if i == 0 {
        return ys[0].is_finite();
    }
    if i >= n {
        return ys[n - 1].is_finite();
    }
    if ts[i] <= ts[i - 1] {
        return ys[i].is_finite();
    }
    ys[i - 1].is_finite() && ys[i].is_finite()
}

/// Uniform grid on `[0, tstop]` with step ~`tstep` (at most `cap` intervals).
pub fn uniform_grid(tstop: f64, tstep: f64, cap: usize) -> Vec<f64> {
    if !(tstop.is_finite() && tstop > 0.0 && tstep.is_finite() && tstep > 0.0) {
        return vec![0.0];
    }
    let n = ((tstop / tstep).ceil() as usize).clamp(1, cap);
    (0..=n).map(|k| tstop * k as f64 / n as f64).collect()
}

/// Comparison grid of a transient over the window `[t0, t1]` (SUITE_SPEC 13.3).
///
/// With a usable `tstep` (`0 < tstep < t1 - t0`) the grid is the nominal output grid `k * tstep` (the instants at
/// which a fixed-step simulator really has samples), plus `t0` when the window starts off-grid (`tstart`) and `t1`
/// when the last step is partial. Otherwise (`tstep >= tstop`, or a parameterised card the harness cannot parse)
/// the grid is the union of the time rows of `axes` inside the window (the caller passes rustspice's own rows). At most `cap` points: a stride thins the
/// grid, the end points are always kept.
///
/// A step so small that `t1 / tstep` exceeds `cap * 2^20` (the `u64` index of the nominal grid would saturate, then
/// overflow) cannot be walked: the grid is `cap` uniform intervals over the window instead, end points included.
pub fn tran_grid(tstep: Option<f64>, t0: f64, t1: f64, axes: &[&[f64]], cap: usize) -> Vec<f64> {
    if !(t0.is_finite() && t1.is_finite() && t1 > t0) {
        return vec![t0.max(0.0)];
    }
    let cap = cap.max(2);
    let mut pts: Vec<f64> = Vec::new();
    // the nominal grid is only walked when its index range is far inside `u64` (and the f64 products `k * h` are exact
    // to well below `h`)
    let walkable = |h: f64| t1.abs().max(t0.abs()) / h <= cap as f64 * 1_048_576.0;
    match tstep.filter(|h| h.is_finite() && *h > 0.0 && *h < t1 - t0) {
        Some(h) if !walkable(h) => {
            let n = cap;
            pts.extend((0..=n).map(|k| {
                if k == n {
                    t1
                } else {
                    t0 + (t1 - t0) * k as f64 / n as f64
                }
            }));
        }
        Some(h) => {
            const EPS: f64 = 1e-9;
            let k_lo = (t0 / h - EPS).ceil().max(0.0) as u64;
            let k_hi = (t1 / h + EPS).floor().max(0.0) as u64;
            if k_hi >= k_lo {
                if t0 < k_lo as f64 * h * (1.0 - EPS) {
                    pts.push(t0);
                }
                let n = (k_hi - k_lo).saturating_add(1);
                let stride = n.div_ceil(cap as u64).max(1);
                let mut k = k_lo;
                while k <= k_hi {
                    pts.push((k as f64 * h).min(t1));
                    match k.checked_add(stride) {
                        Some(next) => k = next,
                        None => break,
                    }
                }
                let last = (k_hi as f64 * h).min(t1);
                if pts.last().is_none_or(|p| *p < last) {
                    pts.push(last);
                }
                if t1 - last > EPS * h {
                    pts.push(t1);
                }
            } else {
                pts.push(t0);
                pts.push(t1);
            }
        }
        None => {
            let span = t1 - t0;
            let mut all: Vec<f64> = axes
                .iter()
                .flat_map(|a| a.iter().copied())
                .filter(|t| *t >= t0 && *t <= t1)
                .collect();
            all.push(t0);
            all.push(t1);
            all.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            all.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * span);
            let stride = all.len().div_ceil(cap).max(1);
            pts = all.iter().copied().step_by(stride).collect();
            if pts.last().is_none_or(|p| *p < t1) {
                pts.push(t1);
            }
        }
    }
    pts
}

/// Rows a fixed-step simulator should write for `.tran tstep tstop` from t = 0: `floor(tstop/tstep + 1e-9) + 1`,
/// plus one for a partial last step.
pub fn expected_tran_rows(tstep: f64, tstop: f64) -> usize {
    let n = (tstop / tstep + 1e-9).floor().max(0.0);
    let partial = tstop - n * tstep > 1e-9 * tstep;
    (n as usize)
        .saturating_add(1)
        .saturating_add(usize::from(partial))
}

/// Linear-interpolation noise of the reference at the grid points: the same reference thinned to every second
/// row, interpolated, versus the full reference (normalised). Small when the reference rows are dense enough for
/// the grid comparison to be trusted.
fn decimation_floor(t: &[f64], y: &[f64], grid: &[f64], denom: f64) -> f64 {
    let n = t.len().min(y.len());
    if n < 5 {
        return 0.0;
    }
    let mut td: Vec<f64> = t[..n].iter().step_by(2).copied().collect();
    let mut yd: Vec<f64> = y[..n].iter().step_by(2).copied().collect();
    if (n - 1) % 2 == 1 {
        td.push(t[n - 1]);
        yd.push(y[n - 1]);
    }
    // At a grid point where the reference has a row of its own the comparison reads that row without interpolation,
    // so there is no resampling noise to estimate: thinning would instead charge it with the interpolation across a
    // corner (a source breakpoint on the output grid) that the comparison never performs.
    let has_row = |g: f64| {
        let i = t[..n].partition_point(|tk| *tk < g);
        [i.checked_sub(1), Some(i)]
            .into_iter()
            .flatten()
            .filter_map(|j| t[..n].get(j))
            .any(|tk| (tk - g).abs() <= 1e-12 * g.abs().max(tk.abs()))
    };
    grid.iter()
        .filter(|&&g| !has_row(g))
        .map(|&g| (interp(&t[..n], &y[..n], g) - interp(&td, &yd, g)).abs())
        .fold(0.0, f64::max)
        / denom
}

fn minmax(v: &[f64]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &x in v {
        if x.is_finite() {
            lo = lo.min(x);
            hi = hi.max(x);
        }
    }
    (lo, hi)
}

/// Times at which `y` crosses `level` (linear interpolation between samples).
pub fn crossings(t: &[f64], y: &[f64], level: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for k in 1..t.len().min(y.len()) {
        let (a, b) = (y[k - 1] - level, y[k] - level);
        if (a < 0.0 && b >= 0.0) || (a > 0.0 && b <= 0.0) {
            let f = a / (a - b);
            out.push(t[k - 1] + f * (t[k] - t[k - 1]));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// Oracle accumulator
// ---------------------------------------------------------------------------------------------------------

struct OracleAcc {
    spec: String,
    /// `ok` | `unsupported` | `panicked` | `budget`
    status: String,
    reason: Option<String>,
    rs_max: Option<f64>,
    ng_max: Option<f64>,
    metric: String,
    budget: oracle::OracleBudget,
    fwd: Option<f64>,
}

impl OracleAcc {
    /// One oracle evaluation under the case's wall-clock budget. After a budget cut-off the oracle is not asked again
    /// for this case (every further probe would spend the same budget again and fail the same way).
    fn eval(&mut self, deck: &str, probe: &str, axis: &[f64]) -> Result<Vec<f64>, (String, bool)> {
        if self.status == "budget" {
            return Err((self.reason.clone().unwrap_or_default(), false));
        }
        let r = catch_unwind(AssertUnwindSafe(|| {
            oracle::evaluate_with_budget(deck, &self.spec, probe, axis, &self.budget)
        }));
        match r {
            Ok(Ok(v)) if v.len() == axis.len() && v.iter().all(|x| x.is_finite()) => {
                bump(&mut self.fwd, oracle::take_forward_error_bound());
                Ok(v)
            }
            Ok(Ok(v)) => Err((
                format!(
                    "oracle returned {} values for {} points (or non-finite)",
                    v.len(),
                    axis.len()
                ),
                false,
            )),
            Ok(Err(e)) => Err((e, false)),
            Err(_) => Err(("oracle panicked".to_string(), true)),
        }
    }

    /// Record the first failure of the oracle for this case.
    fn fail(&mut self, probe: &str, e: String, panicked: bool) {
        if self.status == "ok" {
            self.status = if panicked {
                "panicked"
            } else if oracle::is_budget_error(&e) {
                "budget"
            } else {
                "unsupported"
            }
            .to_string();
            self.reason = Some(format!("{probe}: {e}"));
        }
    }
}

fn bump(slot: &mut Option<f64>, v: Option<f64>) {
    if let Some(v) = v {
        *slot = Some(slot.map_or(v, |s| s.max(v)));
    }
}

/// Probe names to evaluate for one analysis: requested probes, or the `v()` vectors present in both outputs.
fn select_probes(meta: &Meta, rs: &Plot, reference: &Plot, skip_first: bool) -> Vec<String> {
    if !meta.probes.is_empty() {
        return meta.probes.clone();
    }
    let axis = if skip_first {
        reference.vars.first().map(|v| v.name.clone())
    } else {
        None
    };
    reference
        .vars
        .iter()
        .filter(|v| v.name.starts_with("v(") && Some(&v.name) != axis.as_ref())
        .filter(|v| rs.col(&v.name).is_some())
        .map(|v| v.name.clone())
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Pairing of several plots of one analysis kind
// ---------------------------------------------------------------------------------------------------------

/// Plots of `kind` in file order.
fn plots_of(plots: &[Plot], kind: PlotKind) -> Vec<&Plot> {
    plots.iter().filter(|p| p.kind() == Some(kind)).collect()
}

/// Distance between two plots of one kind for the pairing rule (`0` = same span / same values, capped at `1e6`).
fn pair_distance(kind: PlotKind, a: &Plot, b: &Plot) -> f64 {
    const FAR: f64 = 1e6;
    let rel = |x: f64, y: f64| (x - y).abs() / x.abs().max(y.abs()).max(1e-300);
    match kind {
        // the end of the time axis (tstop): ngspice rows start at tstart, RustSpice rows at 0
        PlotKind::Tran => match (
            a.cols.first().and_then(|c| c.last()),
            b.cols.first().and_then(|c| c.last()),
        ) {
            (Some(&x), Some(&y)) if x.is_finite() && y.is_finite() => rel(x, y),
            _ => FAR,
        },
        // first and last sweep value
        PlotKind::Dc => {
            let fl = |p: &Plot| {
                p.cols
                    .first()
                    .and_then(|c| Some((*c.first()?, *c.last()?)))
                    .filter(|(f, l)| f.is_finite() && l.is_finite())
            };
            match (fl(a), fl(b)) {
                (Some((f1, l1)), Some((f2, l2))) => {
                    let scale = f1.abs().max(l1.abs()).max(f2.abs()).max(l2.abs());
                    ((f1 - f2).abs() + (l1 - l2).abs()) / scale.max(1e-300)
                }
                _ => FAR,
            }
        }
        // an operating point has no span: the mean relative difference of the vectors the two plots share
        PlotKind::Op => {
            let (mut sum, mut n) = (0.0, 0usize);
            for (v, col) in a.vars.iter().zip(&a.cols) {
                if let (Some(&x), Some(&y)) = (col.first(), b.col(&v.name).and_then(|c| c.first()))
                {
                    if x.is_finite() && y.is_finite() {
                        sum += rel(x, y);
                        n += 1;
                    }
                }
            }
            if n == 0 {
                FAR
            } else {
                sum / n as f64
            }
        }
    }
}

/// Plot pairing rule (several analyses of one kind in a deck).
///
/// ngspice stores the plots of one kind in reverse card order (checked with the black-box executable: `.dc` with two
/// ranges and two `.op` cards come back newest first) while RustSpice writes them in card order, and the mutation
/// check compares two RustSpice outputs (both in card order). Neither "last plot of the kind" nor "plot k with plot k"
/// is therefore right. Plots are paired by SPAN: the end of the time axis for `.tran`, the first and last sweep value
/// for `.dc`, the values of the shared vectors for `.op` (which has no span). Pairs are assigned greedily by increasing
/// distance (quantised to 1e-9 so that identical spans tie); a tie goes to the HIGHER reference index, which is the
/// ngspice reverse-order convention and, for identical plots, makes no difference to the result.
///
/// Returns `(rustspice index, reference index)` pairs in RustSpice order. Plots that find no partner (one side has
/// more plots than the other) are left out. Bounded: above 40 000 candidate pairs the reverse-order convention is used.
fn pair_plots(kind: PlotKind, rs: &[&Plot], rf: &[&Plot]) -> Vec<(usize, usize)> {
    if rs.len().saturating_mul(rf.len()) > 40_000 {
        let n = rs.len().min(rf.len());
        return (0..n).map(|i| (i, rf.len() - 1 - i)).collect();
    }
    let mut cand: Vec<(u64, std::cmp::Reverse<usize>, usize)> =
        Vec::with_capacity(rs.len() * rf.len());
    for (i, a) in rs.iter().enumerate() {
        for (j, b) in rf.iter().enumerate() {
            let d = pair_distance(kind, a, b);
            let q = (d.clamp(0.0, 1e6) * 1e9).round() as u64;
            cand.push((q, std::cmp::Reverse(j), i));
        }
    }
    cand.sort();
    let (mut used_rs, mut used_rf) = (vec![false; rs.len()], vec![false; rf.len()]);
    let mut pairs = Vec::new();
    for (_, std::cmp::Reverse(j), i) in cand {
        if !used_rs[i] && !used_rf[j] {
            used_rs[i] = true;
            used_rf[j] = true;
            pairs.push((i, j));
        }
    }
    pairs.sort_unstable();
    pairs
}

/// Every `.tran` card of the deck, in card order.
fn tran_cards(deck: &str) -> Vec<TranTokens> {
    deck.lines()
        .enumerate()
        .skip(1)
        .filter(|(_, l)| l.trim_start().to_ascii_lowercase().starts_with(".tran"))
        .filter_map(|(i, l)| {
            find_tran_tokens(&format!("t\n{l}\n")).map(|mut c| {
                c.line = i;
                c
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------------------------------------

pub fn compare(input: &CompareInput) -> CompareResult {
    compare_with(input, None)
}

/// [`compare`] with the row count of the as-is ngspice transient (the `npoints` check accepts it as an
/// alternative to the closed-form row count). `None`: the as-is plot of `input.asis` is used when present.
pub fn compare_with(input: &CompareInput, ng_asis_rows: Option<usize>) -> CompareResult {
    compare_budgeted(input, ng_asis_rows, None)
}

/// [`compare_with`] with an explicit wall-clock budget for each oracle evaluation (`None`:
/// [`oracle::default_wall_budget`], `min(10 s, case timeout / 2)`).
pub fn compare_budgeted(
    input: &CompareInput,
    ng_asis_rows: Option<usize>,
    oracle_wall: Option<Duration>,
) -> CompareResult {
    let kinds = detect_kinds(input.deck);
    let mut res = CompareResult::default();
    if kinds.is_empty() {
        res.notes.push("deck has no .op/.dc/.tran card".to_string());
        res.analysis = "none".to_string();
        return res;
    }
    res.analysis = kinds
        .iter()
        .map(|k| k.as_str())
        .collect::<Vec<_>>()
        .join("+");
    let mut oracle_acc: Option<OracleAcc> = input.meta.oracle.as_ref().map(|s| OracleAcc {
        spec: s.clone(),
        status: "ok".to_string(),
        reason: None,
        rs_max: None,
        ng_max: None,
        metric: if kinds.contains(&PlotKind::Tran) {
            "norm"
        } else {
            "abs"
        }
        .to_string(),
        budget: oracle_wall.map_or_else(oracle::OracleBudget::default, oracle::OracleBudget::new),
        fwd: None,
    });
    let cards = tran_cards(input.deck);
    let mut any_fail = false;
    let mut any_unverifiable = false;
    let mut evaluated = 0usize;
    for &kind in &kinds {
        let rs_all = plots_of(input.rs, kind);
        let rf_all = plots_of(input.reference, kind);
        let ng_all = input.asis.map(|p| plots_of(p, kind)).unwrap_or_default();
        if rs_all.is_empty() || rf_all.is_empty() {
            if rs_all.is_empty() {
                res.failures
                    .push(format!("rustspice wrote no {} plot", kind.as_str()));
                any_fail = true;
            } else {
                res.notes
                    .push(format!("reference has no {} plot", kind.as_str()));
                any_unverifiable = true;
            }
            continue;
        }
        let multi = rs_all.len() > 1 || rf_all.len() > 1;
        // (rustspice idx, reference idx, as-is ngspice plot)
        let mut work: Vec<(usize, usize, Option<&Plot>)> = Vec::new();
        if multi {
            let pairs = pair_plots(kind, &rs_all, &rf_all);
            let ng_pairs = pair_plots(kind, &rf_all, &ng_all);
            for &(i, j) in &pairs {
                let ng = ng_pairs
                    .iter()
                    .find(|(rj, _)| *rj == j)
                    .map(|(_, k)| ng_all[*k]);
                work.push((i, j, ng));
            }
            res.notes.push(format!(
                "{}: rustspice wrote {} plot(s), the reference has {}; paired by span ({})",
                kind.as_str(),
                rs_all.len(),
                rf_all.len(),
                pairs
                    .iter()
                    .map(|(i, j)| format!("rustspice #{} <-> reference #{}", i + 1, j + 1))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if rs_all.len() < rf_all.len() {
                res.failures.push(format!(
                    "rustspice wrote {} {} plot(s), the reference has {}: {} analysis plot(s) are missing",
                    rs_all.len(),
                    kind.as_str(),
                    rf_all.len(),
                    rf_all.len() - rs_all.len()
                ));
                any_fail = true;
            } else if rs_all.len() > rf_all.len() {
                res.notes.push(format!(
                    "{}: {} rustspice plot(s) have no counterpart in the reference and are not compared",
                    kind.as_str(),
                    rs_all.len() - rf_all.len()
                ));
            }
        } else {
            work.push((0, 0, ng_all.last().copied()));
        }
        for (i, j, ng) in work {
            let (rs, rf) = (rs_all[i], rf_all[j]);
            let mut part = match kind {
                PlotKind::Op => compare_op(input, rs, rf, ng, &mut oracle_acc),
                PlotKind::Dc => compare_dc(input, rs, rf, ng, &mut oracle_acc),
                PlotKind::Tran => {
                    // the card of this plot: card order is RustSpice's plot order; otherwise the card whose stop time
                    // is nearest to the end of the reference plot
                    let card = if multi && cards.len() == rs_all.len() {
                        cards.get(i).cloned()
                    } else if multi {
                        let end = rf.cols.first().and_then(|c| c.last()).copied();
                        cards
                            .iter()
                            .filter(|c| c.tstop.is_some())
                            .min_by(|a, b| {
                                let d = |c: &TranTokens| {
                                    (c.tstop.unwrap_or(0.0) - end.unwrap_or(0.0)).abs()
                                };
                                d(a).total_cmp(&d(b))
                            })
                            .cloned()
                    } else {
                        cards.first().cloned()
                    };
                    let rows = if multi {
                        ng.map(|p| p.npoints).or(ng_asis_rows)
                    } else {
                        ng_asis_rows
                    };
                    compare_tran(input, rs, rf, ng, rows, card, &mut oracle_acc)
                }
            };
            if multi {
                let label = format!("{} {}/{}", kind.as_str(), i + 1, rs_all.len());
                for p in &mut part.probes {
                    p.name = format!("{} [{label}]", p.name);
                }
                for f in part.failures.iter_mut().chain(part.notes.iter_mut()) {
                    *f = format!("[{label}] {f}");
                }
            }
            res.points_rs = Some(res.points_rs.unwrap_or(0) + rs.npoints);
            res.points_ref = Some(res.points_ref.unwrap_or(0) + rf.npoints);
            if let Some(f) = part.interp_floor {
                res.interp_floor = Some(res.interp_floor.map_or(f, |x: f64| x.max(f)));
            }
            res.failures.append(&mut part.failures);
            res.notes.append(&mut part.notes);
            if part.structural_fail {
                any_fail = true;
            }
            if part.unverifiable {
                any_unverifiable = true;
            }
            for p in part.probes {
                match p.pass {
                    Some(true) => evaluated += 1,
                    Some(false) => {
                        any_fail = true;
                        evaluated += 1;
                    }
                    None => {
                        if !p.vacuous {
                            any_unverifiable = true;
                        }
                    }
                }
                if p.pass.is_some() || p.vacuous {
                    bump(&mut res.err_rs, if p.vacuous { None } else { p.rs_err });
                    bump(&mut res.err_ng, if p.vacuous { None } else { p.ng_err });
                }
                res.probes.push(p);
            }
        }
    }
    // case-level verdict
    res.vacuous_only = !any_fail
        && !any_unverifiable
        && evaluated == 0
        && !res.probes.is_empty()
        && res.probes.iter().all(|p| p.vacuous);
    res.pass = if any_fail {
        Some(false)
    } else if any_unverifiable {
        None
    } else if evaluated == 0 {
        res.notes
            .push("no non-vacuous probe could be evaluated".to_string());
        None
    } else {
        Some(true)
    };
    let eval: Vec<&ProbeResult> = res
        .probes
        .iter()
        .filter(|p| p.within_ng_error.is_some())
        .collect();
    if !eval.is_empty() {
        res.within_ng_error = Some(eval.iter().all(|p| p.within_ng_error == Some(true)));
    }
    res.oracle = oracle_acc.map(|o| OracleSummary {
        spec: o.spec,
        status: o.status,
        reason: o.reason,
        rs_max_err: o.rs_max,
        ng_max_err: o.ng_max,
        metric: o.metric,
        fwd_bound: o.fwd,
    });
    res
}

#[derive(Default)]
struct Part {
    probes: Vec<ProbeResult>,
    failures: Vec<String>,
    notes: Vec<String>,
    structural_fail: bool,
    /// The reference (not RustSpice) is the side that cannot be used: nothing can be decided, nothing failed.
    unverifiable: bool,
    interp_floor: Option<f64>,
}

fn missing_probe(kind: PlotKind, name: &str, rs: &Plot, rf: &Plot, part: &mut Part) -> bool {
    let (in_rs, in_ref) = (rs.col(name).is_some(), rf.col(name).is_some());
    if in_rs && in_ref {
        return false;
    }
    let mut p = ProbeResult {
        name: name.to_string(),
        analysis: kind.as_str().to_string(),
        ..Default::default()
    };
    if !in_rs && in_ref {
        p.pass = Some(false);
        p.note = Some("missing in rustspice output".to_string());
        part.failures.push(format!(
            "{}: probe {name} missing in rustspice output",
            kind.as_str()
        ));
    } else {
        p.note = Some("missing in reference output".to_string());
        part.notes.push(format!(
            "{}: probe {name} missing in the ngspice reference",
            kind.as_str()
        ));
    }
    part.probes.push(p);
    true
}

// ---------------------------------------------------------------------------------------------------------
// Vacuous probes (SUITE_SPEC 4) and the unit-blind default floor
// ---------------------------------------------------------------------------------------------------------

/// Value of `Meta::defaults().floor`.
const DEFAULT_FLOOR: f64 = 1e-6;

/// `true` when the floor in force is the engine default and the deck did not set `bench.floor` itself.
fn floor_is_default(deck: &str, m: &Meta) -> bool {
    m.floor == DEFAULT_FLOOR
        && !deck.lines().any(|l| {
            l.trim_start()
                .trim_start_matches('*')
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("bench.floor")
        })
}

/// Unit of a probe for messages: the floor is the same number for volts and amperes.
fn unit_of(name: &str) -> &'static str {
    if name.starts_with("i(") {
        "A"
    } else if name.starts_with("v(") {
        "V"
    } else {
        ""
    }
}

enum FloorVerdict {
    /// The reference is ~0 but RustSpice is grossly different (more than `max(tol.abs, floor)`).
    Fail,
    /// The reference is below the DEFAULT floor and RustSpice deviates by more than `tol.abs`: not a verdict either
    /// way, and not allowed to disappear into "vacuous" (a current of 5e-8 A is a real signal, the floor of 1e-6 is
    /// meant for volts). Carries the note.
    Unverified(String),
    /// Reference ~0 and nothing deviates visibly. Carries the note.
    Vacuous(String),
}

/// Policy for a probe whose reference is below the floor. `what` describes the reference (`|ref|=1.500e-7`), `dev` is
/// the largest deviation of RustSpice in probe units. With the default floor (not set by the deck) a deviation above
/// `tol.abs` is unverified, never silently vacuous; a deck that sets `bench.floor` has decided and keeps the vacuous
/// verdict.
fn vacuous_policy(name: &str, what: &str, dev: f64, m: &Meta, floor_default: bool) -> FloorVerdict {
    let u = unit_of(name);
    if dev > m.tol_abs.max(m.floor) {
        return FloorVerdict::Fail;
    }
    if floor_default && dev > m.tol_abs {
        return FloorVerdict::Unverified(format!(
            "unverified: {what} {u} is below the default floor {:.1e} {u} (the floor is unit-blind: it applies to volts and amperes alike) but rustspice deviates by {dev:.3e} {u} > tol.abs {:.1e}; set bench.floor below the signal to decide, or retarget the probe",
            m.floor, m.tol_abs
        ));
    }
    FloorVerdict::Vacuous(format!(
        "vacuous: {what} {u} < floor {:.1e} {u}{}",
        m.floor,
        if floor_default {
            " (default floor, unit-blind: it applies to volts and amperes alike)"
        } else {
            ""
        }
    ))
}

fn compare_op(
    input: &CompareInput,
    rs: &Plot,
    rf: &Plot,
    ng: Option<&Plot>,
    oracle_acc: &mut Option<OracleAcc>,
) -> Part {
    let m = input.meta;
    let floor_default = floor_is_default(input.deck, m);
    let mut part = Part::default();
    for name in select_probes(m, rs, rf, false) {
        if missing_probe(PlotKind::Op, &name, rs, rf, &mut part) {
            continue;
        }
        let r = rf.col(&name).unwrap().first().copied().unwrap_or(f64::NAN);
        let a = rs.col(&name).unwrap().first().copied().unwrap_or(f64::NAN);
        let mut p = ProbeResult {
            name: name.clone(),
            analysis: "op".to_string(),
            ..Default::default()
        };
        if !r.is_finite() {
            p.note = Some("reference value is not finite".to_string());
            part.notes.push(format!(
                "{name}: the reference value is not finite: unverified"
            ));
            part.probes.push(p);
            continue;
        }
        if !a.is_finite() {
            p.pass = Some(false);
            p.note = Some("rustspice value is not finite".to_string());
            part.failures
                .push(format!("{name}: rustspice value is {a}"));
            part.probes.push(p);
            continue;
        }
        let abs = (a - r).abs();
        let tol = m.tol_abs + m.tol_rel * r.abs();
        p.abs_err = Some(abs);
        p.rs_err = Some(abs);
        p.rel_err = Some(abs / r.abs().max(m.floor));
        if tol > 0.0 {
            p.tol_ratio = Some(abs / tol);
        }
        if r.abs() < m.floor {
            p.tol_ratio = None;
            match vacuous_policy(
                &name,
                &format!("|ref|={:.3e}", r.abs()),
                abs,
                m,
                floor_default,
            ) {
                FloorVerdict::Fail => {
                    p.vacuous = true;
                    p.pass = Some(false);
                    p.note = Some(format!(
                        "vacuous: |ref|={:.3e} {} < floor {:.1e}",
                        r.abs(),
                        unit_of(&name),
                        m.floor
                    ));
                    part.failures.push(format!(
                        "{name}: reference ~0 ({:.3e}) but rustspice differs by {abs:.3e}",
                        r
                    ));
                }
                FloorVerdict::Unverified(note) => {
                    p.note = Some(note.clone());
                    part.notes.push(format!("{name}: {note}"));
                }
                FloorVerdict::Vacuous(note) => {
                    p.vacuous = true;
                    p.note = Some(note);
                }
            }
        } else {
            let ok = abs <= tol;
            p.pass = Some(ok);
            if !ok {
                part.failures.push(format!(
                    "{name}: |{a:.9e} - {r:.9e}| = {abs:.3e} > tol {tol:.3e}"
                ));
            }
        }
        if let Some(n) = ng
            .and_then(|n| n.col(&name))
            .and_then(|c| c.first().copied())
        {
            if n.is_finite() {
                p.ng_err = Some((n - r).abs());
            }
        }
        if let (Some(e), Some(ne)) = (p.rs_err, p.ng_err) {
            p.within_ng_error = Some(e <= ne.max(tol));
        }
        if let Some(acc) = oracle_acc.as_mut() {
            match acc.eval(input.deck, &name, &[0.0]) {
                Ok(o) => {
                    p.oracle_rs_err = Some((a - o[0]).abs());
                    p.oracle_ng_err = ng
                        .and_then(|n| n.col(&name))
                        .and_then(|c| c.first())
                        .map(|n| (n - o[0]).abs());
                    bump(&mut acc.rs_max, p.oracle_rs_err);
                    bump(&mut acc.ng_max, p.oracle_ng_err);
                }
                Err((e, panicked)) => acc.fail(&name, e, panicked),
            }
        }
        part.probes.push(p);
    }
    if part.probes.is_empty() {
        part.notes.push("op: no common probes".to_string());
    }
    part
}

/// Evidence for a dc sweep whose grids do not match: compare the leading points on which the two axes ARE aligned and
/// report the per-point errors in the probe records. Never used for the verdict (the structural failure decides).
fn dc_aligned_prefix_evidence(
    input: &CompareInput,
    rs: &Plot,
    rf: &Plot,
    n_aligned: usize,
    part: &mut Part,
) {
    let (Some(ax_rs), Some(ax_rf)) = (rs.cols.first(), rf.cols.first()) else {
        return;
    };
    let n = n_aligned.min(ax_rs.len()).min(ax_rf.len());
    if n == 0 {
        return;
    }
    for name in select_probes(input.meta, rs, rf, true) {
        let (Some(a), Some(r)) = (rs.col(&name), rf.col(&name)) else {
            continue;
        };
        let n = n.min(a.len()).min(r.len());
        let mut p = ProbeResult {
            name: name.clone(),
            analysis: "dc".to_string(),
            ..Default::default()
        };
        // at most 64 listed points (evenly thinned, plus the worst and the last aligned one), collected while scanning
        // so that a sweep of a million points never materialises a million records
        let stride = n.div_ceil(62).max(1);
        let mut listed: Vec<(usize, PointError)> = Vec::new();
        let (mut worst, mut worst_k) = (f64::NEG_INFINITY, None);
        let mut skipped = 0usize;
        let point = |k: usize| PointError {
            at: ax_rf[k],
            rustspice: a[k],
            reference: r[k],
            abs_err: (a[k] - r[k]).abs(),
        };
        for k in 0..n {
            let d = (a[k] - r[k]).abs();
            if !d.is_finite() {
                skipped += 1;
                continue;
            }
            if d > worst {
                worst = d;
                worst_k = Some(k);
            }
            if k % stride == 0 || k + 1 == n {
                listed.push((k, point(k)));
            }
        }
        if let Some(wk) = worst_k {
            if !listed.iter().any(|(k, _)| *k == wk) {
                listed.push((wk, point(wk)));
                listed.sort_by_key(|(k, _)| *k);
            }
        }
        let pts: Vec<PointError> = listed.into_iter().map(|(_, e)| e).collect();
        let used = n - skipped;
        p.aligned_points = Some(n);
        p.aligned_abs_err = (used > 0).then_some(worst);
        p.aligned_worst_at = worst_k.map(|k| ax_rf[k]);
        p.point_errors = pts;
        p.note = Some(format!(
            "grids differ (rustspice {} points, reference {}): evidence on the {n} aligned leading point(s){}: max |err| {}{}; not part of the verdict",
            ax_rs.len(),
            ax_rf.len(),
            if skipped > 0 {
                format!(" ({skipped} non-finite skipped)")
            } else {
                String::new()
            },
            p.aligned_abs_err
                .map_or("n/a".to_string(), |e| format!("{e:.3e}")),
            p.aligned_worst_at
                .map_or(String::new(), |x| format!(" at sweep {x:.6e}")),
        ));
        part.probes.push(p);
    }
}

fn compare_dc(
    input: &CompareInput,
    rs: &Plot,
    rf: &Plot,
    ng: Option<&Plot>,
    oracle_acc: &mut Option<OracleAcc>,
) -> Part {
    let m = input.meta;
    let floor_default = floor_is_default(input.deck, m);
    let mut part = Part::default();
    let (Some(ax_rs), Some(ax_rf)) = (rs.cols.first(), rf.cols.first()) else {
        part.structural_fail = true;
        part.failures.push("dc: plot has no sweep axis".to_string());
        return part;
    };
    let (lo, hi) = minmax(ax_rf);
    let span = (hi - lo).abs();
    let aligned =
        |a: &f64, b: &f64| (a - b).abs() <= 1e-9 * span.max(a.abs()).max(b.abs()).max(1e-30);
    if ax_rs.len() != ax_rf.len() {
        part.structural_fail = true;
        part.failures.push(format!(
            "dc: point-count mismatch (rustspice {} vs reference {})",
            ax_rs.len(),
            ax_rf.len()
        ));
        let n_aligned = ax_rs
            .iter()
            .zip(ax_rf.iter())
            .take_while(|(a, b)| aligned(a, b))
            .count();
        dc_aligned_prefix_evidence(input, rs, rf, n_aligned, &mut part);
        return part;
    }
    for (k, (a, b)) in ax_rs.iter().zip(ax_rf.iter()).enumerate() {
        if !aligned(a, b) {
            part.structural_fail = true;
            part.failures.push(format!(
                "dc: sweep grids differ at point {k} ({a:e} vs {b:e})"
            ));
            dc_aligned_prefix_evidence(input, rs, rf, k, &mut part);
            return part;
        }
    }
    let ng_axis_ok = ng
        .and_then(|n| n.cols.first())
        .is_some_and(|ax| ax.len() == ax_rf.len());
    let probes = select_probes(m, rs, rf, true);
    for name in probes {
        if missing_probe(PlotKind::Dc, &name, rs, rf, &mut part) {
            continue;
        }
        let (a, r) = (rs.col(&name).unwrap(), rf.col(&name).unwrap());
        let mut p = ProbeResult {
            name: name.clone(),
            analysis: "dc".to_string(),
            ..Default::default()
        };
        let mut worst = 0.0f64;
        let mut worst_at = None;
        let mut worst_rel = 0.0f64;
        let mut worst_ratio = 0.0f64;
        let mut nonfinite = false;
        let mut skipped_ref = 0usize;
        let mut rmax = 0.0f64;
        for k in 0..r.len() {
            // a reference point that is not finite cannot be compared: counted and reported, never silently skipped
            if !r[k].is_finite() {
                skipped_ref += 1;
                continue;
            }
            if !a[k].is_finite() {
                nonfinite = true;
                continue;
            }
            let d = (a[k] - r[k]).abs();
            rmax = rmax.max(r[k].abs());
            if d >= worst {
                worst = d;
                worst_at = Some(ax_rf[k]);
            }
            worst_rel = worst_rel.max(d / r[k].abs().max(m.floor));
            worst_ratio = worst_ratio.max(d / (m.tol_abs + m.tol_rel * r[k].abs()));
        }
        let compared = r.len() - skipped_ref;
        if compared > 0 {
            p.abs_err = Some(worst);
            p.rs_err = Some(worst);
            p.rel_err = Some(worst_rel);
            p.worst_at = worst_at;
            p.tol_ratio = Some(worst_ratio);
        }
        if nonfinite {
            p.pass = Some(false);
            p.note = Some("rustspice output contains non-finite values".to_string());
            part.failures.push(format!(
                "{name}: rustspice output contains non-finite values"
            ));
        } else if compared == 0 && skipped_ref > 0 {
            // an all-non-finite reference: nothing was compared (no error numbers, no vacuous pass)
            p.note = Some(format!(
                "reference is not finite at all {skipped_ref} point(s): nothing to compare"
            ));
        } else if rmax < m.floor {
            p.tol_ratio = None;
            match vacuous_policy(
                &name,
                &format!("max|ref|={rmax:.3e}"),
                worst,
                m,
                floor_default,
            ) {
                FloorVerdict::Fail => {
                    p.vacuous = true;
                    p.pass = Some(false);
                    p.note = Some(format!(
                        "vacuous: max|ref|={rmax:.3e} {} < floor {:.1e}",
                        unit_of(&name),
                        m.floor
                    ));
                    part.failures.push(format!(
                        "{name}: reference ~0 over the sweep but rustspice differs by {worst:.3e}"
                    ));
                }
                FloorVerdict::Unverified(note) => {
                    p.note = Some(note.clone());
                    part.notes.push(format!("{name}: {note}"));
                }
                FloorVerdict::Vacuous(note) => {
                    p.vacuous = true;
                    p.note = Some(note);
                }
            }
        } else {
            let ok = worst_ratio <= 1.0;
            p.pass = Some(ok);
            if !ok {
                part.failures.push(format!(
                    "{name}: max |err| {worst:.3e} at sweep {} exceeds tolerance (worst/tol = {worst_ratio:.3e})",
                    worst_at.map_or("?".to_string(), |x| format!("{x:.6e}"))
                ));
            }
        }
        if skipped_ref > 0 {
            let msg = format!(
                "{skipped_ref} of {} reference points are not finite and were skipped: the probe cannot pass",
                r.len()
            );
            part.notes.push(format!("{name}: {msg}"));
            if p.pass == Some(true) || (p.pass.is_none() && p.note.is_none()) {
                p.pass = None;
                p.note = Some(msg);
            } else if let Some(n) = p.note.as_mut() {
                n.push_str(&format!("; {msg}"));
            }
        }
        if ng_axis_ok {
            if let Some(n) = ng.and_then(|n| n.col(&name)) {
                let d = n
                    .iter()
                    .zip(r.iter())
                    .filter(|(x, y)| x.is_finite() && y.is_finite())
                    .map(|(x, y)| (x - y).abs())
                    .fold(0.0, f64::max);
                p.ng_err = Some(d);
            }
        }
        if let (Some(e), Some(ne)) = (p.rs_err, p.ng_err) {
            p.within_ng_error = Some(e <= ne.max(m.tol_abs + m.tol_rel * rmax));
        }
        if let Some(acc) = oracle_acc.as_mut() {
            match acc.eval(input.deck, &name, ax_rf) {
                Ok(o) => {
                    p.oracle_rs_err = Some(
                        a.iter()
                            .zip(&o)
                            .map(|(x, y)| (x - y).abs())
                            .fold(0.0, f64::max),
                    );
                    if ng_axis_ok {
                        p.oracle_ng_err = ng.and_then(|n| n.col(&name)).map(|n| {
                            n.iter()
                                .zip(&o)
                                .map(|(x, y)| (x - y).abs())
                                .fold(0.0, f64::max)
                        });
                    }
                    bump(&mut acc.rs_max, p.oracle_rs_err);
                    bump(&mut acc.ng_max, p.oracle_ng_err);
                }
                Err((e, panicked)) => acc.fail(&name, e, panicked),
            }
        }
        part.probes.push(p);
    }
    if part.probes.is_empty() {
        part.notes.push("dc: no common probes".to_string());
    }
    part
}

/// Max |y_a - y_b| over a grid and its location. A NaN difference (an infinity minus an infinity) means the point
/// could not be compared and makes the result NaN: it is never dropped (`d >= worst` is false for NaN).
fn max_abs_diff(grid: &[f64], a: &[f64], b: &[f64]) -> (f64, f64) {
    let mut worst = 0.0f64;
    let mut at = grid.first().copied().unwrap_or(0.0);
    for k in 0..grid.len() {
        let d = (a[k] - b[k]).abs();
        if d.is_nan() {
            return (f64::NAN, grid[k]);
        }
        if d >= worst {
            worst = d;
            at = grid[k];
        }
    }
    (worst, at)
}

/// Difference statistics over the grid points where BOTH values are finite.
struct DiffStats {
    worst: f64,
    at: f64,
    rms: f64,
    /// Points compared.
    used: usize,
}

fn diff_stats(grid: &[f64], a: &[f64], b: &[f64]) -> DiffStats {
    let (mut worst, mut at, mut sq, mut used) =
        (0.0f64, grid.first().copied().unwrap_or(0.0), 0.0, 0usize);
    for k in 0..grid.len() {
        if !(a[k].is_finite() && b[k].is_finite()) {
            continue;
        }
        let d = (a[k] - b[k]).abs();
        sq += d * d;
        used += 1;
        if d >= worst {
            worst = d;
            at = grid[k];
        }
    }
    DiffStats {
        worst,
        at,
        rms: if used > 0 {
            (sq / used as f64).sqrt()
        } else {
            f64::NAN
        },
        used,
    }
}

/// `x < limit`, or `x` is NaN: "does not reach the limit" (a NaN never reaches anything, `!(x >= limit)` spelled
/// without the negated partial comparison).
fn not_reaching(x: f64, limit: f64) -> bool {
    !matches!(
        x.partial_cmp(&limit),
        Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
    )
}

/// `x > y` and neither is NaN.
fn exceeds(x: f64, y: f64) -> bool {
    matches!(x.partial_cmp(&y), Some(std::cmp::Ordering::Greater))
}

/// First row where a time axis is not finite or decreases (`None`: fine).
fn axis_defect(t: &[f64]) -> Option<(usize, &'static str)> {
    for (k, x) in t.iter().enumerate() {
        if !x.is_finite() {
            return Some((k, "is not finite"));
        }
        if k > 0 && *x < t[k - 1] {
            return Some((k, "decreases"));
        }
    }
    None
}

/// At most `cap` indices of `0..n`, evenly spread, always including the first and the last.
fn thin_indices(n: usize, cap: usize) -> Vec<usize> {
    if n <= cap {
        return (0..n).collect();
    }
    let stride = n.div_ceil(cap.max(2));
    let mut v: Vec<usize> = (0..n).step_by(stride).collect();
    if v.last() != Some(&(n - 1)) {
        v.push(n - 1);
    }
    v
}

fn compare_tran(
    input: &CompareInput,
    rs: &Plot,
    rf: &Plot,
    ng: Option<&Plot>,
    ng_asis_rows: Option<usize>,
    card: Option<TranTokens>,
    oracle_acc: &mut Option<OracleAcc>,
) -> Part {
    let m = input.meta;
    let floor_default = floor_is_default(input.deck, m);
    let mut part = Part::default();
    let Some(card) = card else {
        part.structural_fail = true;
        part.failures
            .push("tran: deck has no parsable .tran card".to_string());
        return part;
    };
    let (Some(t_rs), Some(t_rf)) = (rs.cols.first(), rf.cols.first()) else {
        part.structural_fail = true;
        part.failures
            .push("tran: plot has no time axis".to_string());
        return part;
    };
    if rs.vars[0].name != "time" || rf.vars[0].name != "time" {
        part.structural_fail = true;
        part.failures
            .push("tran: first vector is not `time`".to_string());
        return part;
    }
    // Both time axes must be finite and non-decreasing before anything is interpolated: `interp` and the window tests
    // silently turn a NaN axis into "agrees" (every comparison with NaN is false).
    if let Some((k, why)) = axis_defect(t_rs) {
        part.structural_fail = true;
        part.failures.push(format!(
            "tran: rustspice time axis {why} at row {k} (t = {:e})",
            t_rs[k]
        ));
        return part;
    }
    if let Some((k, why)) = axis_defect(t_rf) {
        part.unverifiable = true;
        part.notes.push(format!(
            "tran: the reference time axis {why} at row {k} (t = {:e}): the transient cannot be compared; unverified",
            t_rf[k]
        ));
        return part;
    }
    let end_rs = t_rs.last().copied().unwrap_or(0.0);
    let end_rf = t_rf.last().copied().unwrap_or(0.0);
    // window: from the first ngspice row (t >= tstart) to tstop; a parameterised card takes its end from ngspice
    let t0 = t_rf.first().copied().unwrap_or(0.0).max(0.0);
    let parsed_stop = card.tstop.filter(|x| x.is_finite());
    let tstop = parsed_stop.unwrap_or(end_rf);
    if parsed_stop.is_none() {
        part.notes.push(format!(
            "tran: stop time `{}` is not a plain number; the window ends at the last ngspice row",
            card.tstop_tok
        ));
    }
    let mut tend = tstop;
    if not_reaching(end_rs, tstop * (1.0 - 1e-6)) {
        part.structural_fail = true;
        part.failures.push(format!(
            "tran: rustspice ends at t={end_rs:.6e} < tstop {tstop:.6e}"
        ));
        tend = tend.min(end_rs);
    }
    // a reference with fewer than two rows (or no extent) is the reference's defect, not RustSpice's
    if t_rf.len() < 2 || !exceeds(end_rf, t0) {
        part.unverifiable = true;
        part.notes.push(format!(
            "tran: the reference has {} row(s) ending at t={end_rf:.6e}: nothing to compare against; unverified",
            t_rf.len()
        ));
        return part;
    }
    if not_reaching(end_rf, tstop * (1.0 - 1e-6)) {
        part.notes
            .push(format!("tran: reference ends at t={end_rf:.6e} < tstop"));
        tend = tend.min(end_rf);
    }
    if tend.partial_cmp(&t0) != Some(std::cmp::Ordering::Greater) {
        part.structural_fail = true;
        part.failures.push("tran: no common time range".to_string());
        return part;
    }
    // `.tran` with a start time: ngspice rows start there, rustspice rows start at 0 and are compared on the window
    if t0 > 0.0 {
        part.notes.push(format!(
            "tran: compared on [{t0:.6e}, {tend:.6e}] (first ngspice row; tstart honoured)"
        ));
    }
    // tstep >= tstop or an unparsable tstep: the nominal grid degenerates, so the instants at which rustspice has
    // samples (its own rows, always including t0 and tend) are compared; interpolating a one-step waveform onto
    // ngspice's internal time points would measure the interpolation, not the simulator
    let grid = tran_grid(card.tstep, t0, tend, &[t_rs], 200_000);
    if card.tstep.is_none_or(|h| !(h > 0.0 && h < tend - t0)) {
        part.notes.push(format!(
            "tran: tstep `{}` does not subdivide the window; compared at the rustspice time rows ({} points)",
            card.tstep_tok,
            grid.len()
        ));
    }
    // RustSpice's own rows inside the window (at most 200 000): where the own-grid evidence is evaluated
    let own_rows: Vec<usize> = {
        // the axis is sorted (checked above): the rows inside the window are one contiguous range
        let lo_i = t_rs.partition_point(|t| *t < t0);
        let hi_i = t_rs.partition_point(|t| *t <= tend);
        thin_indices(hi_i.saturating_sub(lo_i), 200_000)
            .into_iter()
            .map(|i| lo_i + i)
            .collect()
    };
    let own_t: Vec<f64> = own_rows.iter().map(|&k| t_rs[k]).collect();
    let ng_t = ng.and_then(|n| n.cols.first());
    let want_edge = m.has_tag("timing_edge");
    for name in select_probes(m, rs, rf, false) {
        if missing_probe(PlotKind::Tran, &name, rs, rf, &mut part) {
            continue;
        }
        let (y_rs, y_rf) = (rs.col(&name).unwrap(), rf.col(&name).unwrap());
        let mut p = ProbeResult {
            name: name.clone(),
            analysis: "tran".to_string(),
            ..Default::default()
        };
        // RustSpice values that are not finite where the reference is finite are a violation; where the reference is
        // not finite either nothing can be said (the reference defect decides below)
        let rs_bad = (0..y_rs.len().min(t_rs.len()))
            .any(|k| !y_rs[k].is_finite() && finite_at(t_rf, y_rf, t_rs[k]));
        if rs_bad || (y_rs.iter().any(|x| !x.is_finite()) && y_rf.iter().all(|x| x.is_finite())) {
            p.pass = Some(false);
            p.note = Some("rustspice output contains non-finite values".to_string());
            part.failures.push(format!(
                "{name}: rustspice output contains non-finite values"
            ));
            part.probes.push(p);
            continue;
        }
        let a: Vec<f64> = grid.iter().map(|&t| interp(t_rs, y_rs, t)).collect();
        let r: Vec<f64> = grid.iter().map(|&t| interp(t_rf, y_rf, t)).collect();
        // swing of the reference inside the compared window (finite rows only)
        let (lo, hi) = {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for (t, y) in t_rf.iter().zip(y_rf.iter()) {
                if *t >= t0 && *t <= tend && y.is_finite() {
                    lo = lo.min(*y);
                    hi = hi.max(*y);
                }
            }
            if lo > hi {
                minmax(y_rf)
            } else {
                (lo, hi)
            }
        };
        let ref_rows_bad = t_rf
            .iter()
            .zip(y_rf.iter())
            .filter(|(t, y)| **t >= t0 && **t <= tend && !y.is_finite())
            .count();
        let stats = diff_stats(&grid, &a, &r);
        let skipped = grid.len() - stats.used;
        if stats.used == 0 || lo > hi {
            // nothing finite to compare with: no error numbers (not 0), no pass
            p.note = Some(format!(
                "the reference is not finite on the compared window ({skipped} of {} grid points): nothing to compare",
                grid.len()
            ));
            part.notes
                .push(format!("{name}: {}", p.note.as_deref().unwrap_or("")));
            part.probes.push(p);
            continue;
        }
        let swing = hi - lo;
        let denom = swing.max(m.floor);
        let (worst, at) = (stats.worst, stats.at);
        p.abs_err = Some(worst);
        p.worst_at = Some(at);
        p.norm_err = Some(worst / denom);
        p.rms_norm_err = Some(stats.rms / denom);
        p.rs_err = p.norm_err;
        p.rel_err = Some(worst / hi.abs().max(lo.abs()).max(m.floor));
        let fl = decimation_floor(t_rf, y_rf, &grid, denom);
        p.interp_floor = Some(fl);
        part.interp_floor = Some(part.interp_floor.map_or(fl, |x: f64| x.max(fl)));
        // vacuous = the reference is indistinguishable from 0 (SUITE_SPEC section 4); a flat but non-zero
        // reference is a real check whose error is normalised by the floor instead of the (zero) swing
        if hi.abs().max(lo.abs()) < m.floor {
            p.tol_ratio = None;
            let mag = hi.abs().max(lo.abs());
            match vacuous_policy(
                &name,
                &format!("max|ref|={mag:.3e}"),
                worst,
                m,
                floor_default,
            ) {
                FloorVerdict::Fail => {
                    p.vacuous = true;
                    p.pass = Some(false);
                    p.note = Some(format!(
                        "vacuous: max|ref|={mag:.3e} {} < floor {:.1e}",
                        unit_of(&name),
                        m.floor
                    ));
                    part.failures.push(format!(
                        "{name}: reference ~0 but rustspice deviates by {worst:.3e}"
                    ));
                }
                FloorVerdict::Unverified(note) => {
                    p.note = Some(note.clone());
                    part.notes.push(format!("{name}: {note}"));
                }
                FloorVerdict::Vacuous(note) => {
                    p.vacuous = true;
                    p.note = Some(note);
                }
            }
        } else {
            let ok = worst / denom <= m.tol_norm;
            p.pass = Some(ok);
            p.tol_ratio = Some(worst / denom / m.tol_norm);
            // SUITE_SPEC 14.3: when the reference's own resampling noise exceeds the tolerance, only an error that
            // is further than that noise from the limit (on either side) is a verdict; anything closer is
            // `unverified` with this reason. The remark is never attached to a decided probe.
            let e = worst / denom;
            if fl > m.tol_norm && (e - m.tol_norm).abs() < fl {
                p.pass = None;
                p.note = Some(format!(
                    "resampling floor {fl:.2e} exceeds tol.norm {:.1e} and |err {e:.2e} - tol| < floor",
                    m.tol_norm
                ));
                part.notes.push(format!(
                    "{name}: the reference's resampling floor {fl:.2e} exceeds tol.norm {:.1e} and |err {e:.2e} - tol| < floor: the check cannot decide this probe (denser reference rows, e.g. a smaller `bench.ref.tmax`, would resolve it)",
                    m.tol_norm
                ));
            } else if !ok {
                part.failures.push(format!(
                    "{name}: norm_err {:.3e} > tol {:.1e} (max |err| {worst:.3e} at t={at:.4e}, swing {swing:.3e})",
                    worst / denom,
                    m.tol_norm
                ));
            }
        }
        // grid points whose reference is not finite were skipped: whatever was compared agrees, but the rest is
        // unchecked, so the probe cannot pass (a violation on the finite part above stays a failure)
        if skipped > 0 || ref_rows_bad > 0 {
            let msg = format!(
                "{skipped} of {} grid points ({ref_rows_bad} reference rows) are not finite and were skipped: the probe cannot pass",
                grid.len()
            );
            part.notes.push(format!("{name}: {msg}"));
            if p.pass == Some(true) || (p.pass.is_none() && p.note.is_none()) {
                p.pass = None;
                p.note = Some(msg);
            } else if let Some(n) = p.note.as_mut() {
                n.push_str(&format!("; {msg}"));
            }
        }
        // EVIDENCE, never used for pass/fail: RustSpice at its own rows against the reference
        if !own_t.is_empty() {
            let ya: Vec<f64> = own_rows.iter().map(|&k| y_rs[k]).collect();
            let yr: Vec<f64> = own_t.iter().map(|&t| interp(t_rf, y_rf, t)).collect();
            let s = diff_stats(&own_t, &ya, &yr);
            if s.used > 0 {
                p.own_grid_err = Some(s.worst / denom);
            }
        }
        let mut ng_grid: Option<Vec<f64>> = None;
        if let (Some(nt), Some(ny)) = (ng_t, ng.and_then(|n| n.col(&name))) {
            if nt.len() > 1 && nt.last().copied().unwrap_or(0.0) >= tend * (1.0 - 1e-6) {
                let g: Vec<f64> = grid.iter().map(|&t| interp(nt, ny, t)).collect();
                let s = diff_stats(&grid, &g, &r);
                if s.used > 0 {
                    p.ng_err = Some(s.worst / denom);
                }
                ng_grid = Some(g);
            }
        }
        if let (Some(e), Some(ne)) = (p.rs_err, p.ng_err) {
            p.within_ng_error = Some(e <= ne.max(m.tol_norm));
        }
        if want_edge {
            let level = 0.5 * (lo + hi);
            let (cr, cs) = (crossings(t_rf, y_rf, level), crossings(t_rs, y_rs, level));
            let rf_at_rows: Vec<f64> = t_rs.iter().map(|&t| interp(t_rf, y_rf, t)).collect();
            let ca = crossings(t_rs, &rf_at_rows, level);
            let in_win = |v: Vec<f64>| -> Vec<f64> {
                v.into_iter().filter(|t| *t >= t0 && *t <= tend).collect()
            };
            let (cr, cs, ca) = (in_win(cr), in_win(cs), in_win(ca));
            let max_shift = |a: &[f64], b: &[f64]| {
                a.iter()
                    .zip(b)
                    .map(|(x, y)| (x - y).abs())
                    .fold(0.0, f64::max)
            };
            if cr.len() != cs.len() {
                p.note = Some(format!(
                    "50% crossings: rustspice {} vs reference {}",
                    cs.len(),
                    cr.len()
                ));
            } else if !cr.is_empty() {
                p.edge_shift = Some(max_shift(&cr, &cs));
                if ca.len() == cs.len() {
                    p.edge_shift_at_rows = Some(max_shift(&ca, &cs));
                    p.edge_row_floor = Some(max_shift(&cr, &ca));
                }
            }
        }
        if let Some(acc) = oracle_acc.as_mut() {
            match acc.eval(input.deck, &name, &grid) {
                Ok(o) => {
                    p.oracle_rs_err = Some(max_abs_diff(&grid, &a, &o).0 / denom);
                    p.oracle_ng_err = ng_grid
                        .as_ref()
                        .map(|g| max_abs_diff(&grid, g, &o).0 / denom);
                    bump(&mut acc.rs_max, p.oracle_rs_err);
                    bump(&mut acc.ng_max, p.oracle_ng_err);
                    // evidence: the oracle at RustSpice's own rows, only where there is something to explain
                    // (a probe within half the tolerance needs no second integration of the oracle)
                    let near = p.tol_ratio.is_none_or(|x| x > 0.5) || p.pass != Some(true);
                    if near && own_t.len() >= 2 && own_t != grid {
                        if let Ok(oo) = acc.eval(input.deck, &name, &own_t) {
                            let ya: Vec<f64> = own_rows.iter().map(|&k| y_rs[k]).collect();
                            p.oracle_own_grid_err = Some(max_abs_diff(&own_t, &ya, &oo).0 / denom);
                        }
                    }
                }
                Err((e, panicked)) => acc.fail(&name, e, panicked),
            }
        }
        part.probes.push(p);
    }
    if part.probes.is_empty() {
        part.notes.push("tran: no common probes".to_string());
    }
    if m.has_tag("npoints") {
        check_row_count(&card, rs, ng_asis_rows, &mut part);
    }
    part
}

/// Tag `npoints`: the RustSpice row count must equal the as-is ngspice row count or the closed-form count of a
/// fixed-step run (`floor(tstop/tstep + 1e-9) + 1`, plus one for a partial last step).
fn check_row_count(
    card: &super::meta::TranTokens,
    rs: &Plot,
    ng_rows: Option<usize>,
    part: &mut Part,
) {
    let (Some(h), Some(stop)) = (card.tstep, card.tstop) else {
        part.notes
            .push("npoints: the .tran card is not numeric; row count not checked".to_string());
        return;
    };
    if card.tstart > 0.0 || !(h > 0.0 && stop > 0.0) {
        part.notes
            .push("npoints: not checked for a .tran with a start time".to_string());
        return;
    }
    let expected = expected_tran_rows(h, stop);
    let rows = rs.npoints;
    if rows == expected || Some(rows) == ng_rows {
        part.notes.push(format!(
            "npoints: rustspice wrote {rows} rows (closed form {expected}, ngspice as-is {})",
            ng_rows.map_or("n/a".to_string(), |n| n.to_string())
        ));
    } else {
        part.structural_fail = true;
        part.failures.push(format!(
            "tran: rustspice wrote {rows} rows; expected {expected} (floor(tstop/tstep)+1, +1 for a partial last step) or the ngspice as-is row count {}",
            ng_rows.map_or("n/a".to_string(), |n| n.to_string())
        ));
    }
}

// ---------------------------------------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------------------------------------

/// State of ngspice for a case (the reference run, or the as-is run when it served as the reference).
#[derive(Clone, Debug, PartialEq)]
pub enum NgSide {
    /// ngspice is not installed.
    Missing,
    Ok,
    Error(String),
    Timeout,
    /// ngspice was deliberately not run (tag `ngspice_unbounded`): RustSpice is judged against `bench.expect`
    /// alone, there is no reference to compare with.
    Skipped,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RsSide {
    Ok,
    LoudError(String),
    Panic(String),
    Timeout,
}

#[derive(Clone, Debug)]
pub struct Verdict {
    pub status: Status,
    pub outcome: Option<Outcome>,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Violation {
    Panic,
    Timeout,
    FailAccuracy,
    FailRobustness,
    FailSilent,
}

/// The only `known_gap` slugs under which a RustSpice panic / abort / timeout stays `known_gap_confirmed`: resource
/// bombs and the symbolic-LU blow-up are crashes by nature. SUITE_SPEC section 5: a panic, abort or timeout is a
/// robustness violation; a deck tagged with a slug that describes a loud refusal or wrong numbers (`clamp-rail`,
/// `silent-ignore-directive`, ...) that starts to crash is a NEW failure, not the documented gap, and must not stay green.
pub const CRASH_GAP_SLUGS: [&str; 2] = ["resource-budget", "symbolic-lu-blowup"];

/// `true` when `slug` may absorb a panic or a timeout.
pub fn gap_covers_crash(slug: &str) -> bool {
    CRASH_GAP_SLUGS.contains(&slug)
}

/// Apply SUITE_SPEC section 5. `cmp_pass` is `Some(true|false)` when the accuracy comparison ran, `None` when it
/// could not be evaluated.
pub fn classify(meta: &Meta, ng: &NgSide, rs: &RsSide, cmp_pass: Option<bool>) -> Verdict {
    let mut notes = Vec::new();
    if *ng == NgSide::Missing {
        return Verdict {
            status: Status::Unverified,
            outcome: None,
            notes: vec!["ngspice is not available".to_string()],
        };
    }
    let ng_ok = *ng == NgSide::Ok;
    let ng_skipped = *ng == NgSide::Skipped;
    let mut violation: Option<Violation> = None;
    let mut base_status = Status::Pass;
    let outcome: Option<Outcome>;
    match rs {
        RsSide::Panic(m) => {
            violation = Some(Violation::Panic);
            outcome = Some(Outcome::Panic);
            notes.push(m.clone());
        }
        RsSide::Timeout => {
            violation = Some(Violation::Timeout);
            outcome = Some(Outcome::Timeout);
        }
        RsSide::LoudError(m) => {
            notes.push(format!("rustspice refused: {m}"));
            match meta.expect {
                Expect::LoudError => {
                    outcome = Some(Outcome::LoudError);
                }
                Expect::Match if ng_skipped => {
                    base_status = Status::Unverified;
                    outcome = Some(Outcome::LoudError);
                    notes.push("no reference: ngspice was not run on this deck".to_string());
                }
                Expect::Match => {
                    if ng_ok {
                        violation = Some(Violation::FailRobustness);
                        outcome = Some(Outcome::LoudError);
                    } else {
                        base_status = Status::BothError;
                        outcome = Some(Outcome::BothError);
                    }
                }
                Expect::Either => {
                    base_status = Status::Info;
                    outcome = Some(if ng_ok || ng_skipped {
                        Outcome::LoudError
                    } else {
                        Outcome::BothError
                    });
                }
            }
        }
        RsSide::Ok => match meta.expect {
            Expect::LoudError => {
                violation = Some(Violation::FailSilent);
                outcome = Some(Outcome::SilentNumbers);
                notes.push("rustspice produced numbers where a refusal was expected".to_string());
            }
            Expect::Match if ng_skipped => {
                base_status = Status::Unverified;
                outcome = None;
                notes.push("no reference: ngspice was not run on this deck".to_string());
            }
            Expect::Match => {
                if ng_ok {
                    match cmp_pass {
                        Some(true) => outcome = Some(Outcome::Match),
                        Some(false) => {
                            violation = Some(Violation::FailAccuracy);
                            outcome = Some(Outcome::Mismatch);
                        }
                        None => {
                            base_status = Status::Unverified;
                            outcome = None;
                            notes.push("accuracy comparison could not be evaluated".to_string());
                        }
                    }
                } else {
                    base_status = Status::NgspiceError;
                    outcome = Some(Outcome::NgspiceOnlyError);
                }
            }
            Expect::Either => {
                base_status = Status::Info;
                outcome = if ng_skipped {
                    None
                } else if !ng_ok {
                    Some(Outcome::NgspiceOnlyError)
                } else {
                    match cmp_pass {
                        Some(true) => Some(Outcome::Match),
                        Some(false) => Some(Outcome::Mismatch),
                        None => None,
                    }
                };
                if ng_ok && cmp_pass.is_none() {
                    notes.push("accuracy comparison could not be evaluated".to_string());
                }
            }
        },
    }
    let status = match violation {
        Some(v) => {
            let crash = matches!(v, Violation::Panic | Violation::Timeout);
            let covered = match meta.known_gap.as_deref() {
                None => false,
                Some(slug) if !crash || gap_covers_crash(slug) => true,
                Some(slug) => {
                    notes.push(format!(
                        "known_gap `{slug}` does not cover a panic/abort/timeout (only {} do): a robustness violation",
                        CRASH_GAP_SLUGS.join(", ")
                    ));
                    false
                }
            };
            if covered {
                Status::KnownGapConfirmed
            } else {
                match v {
                    Violation::Panic => Status::Panic,
                    Violation::Timeout => Status::Timeout,
                    Violation::FailAccuracy => Status::FailAccuracy,
                    Violation::FailRobustness => Status::FailRobustness,
                    Violation::FailSilent => Status::FailSilent,
                }
            }
        }
        None => {
            if let Some(slug) = meta.known_gap.as_deref() {
                match base_status {
                    // `expect: either` cannot be violated except by a crash: under a slug that does not describe a
                    // crash the gap can neither be confirmed nor closed, so it must not be reported as closed
                    Status::Pass | Status::Info
                        if meta.expect == Expect::Either && !gap_covers_crash(slug) =>
                    {
                        notes.push(format!(
                            "known_gap `{slug}` with expect=either can never be violated (only a panic or timeout could): contradictory metadata, the gap is neither confirmed nor closed"
                        ));
                        Status::Info
                    }
                    Status::Pass | Status::Info => Status::GapClosed,
                    other => {
                        notes
                            .push("known_gap could not be judged (no ngspice verdict)".to_string());
                        other
                    }
                }
            } else {
                base_status
            }
        }
    };
    Verdict {
        status,
        outcome,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::super::meta::{parse, Meta};
    use super::super::rawfile::Var;
    use super::*;

    fn meta_from(extra: &str) -> Meta {
        let d = format!("t\n* bench.id: x\n* bench.group: accuracy\n{extra}\n.op\n.end\n");
        let p = parse(&d, Some("x"));
        assert!(p.diags.is_empty(), "{:?}", p.diags);
        p.meta
    }

    /// Metadata the strict parser rejects (`known_gap` with `expect: either`): the verdict code must still never read
    /// `gap_closed` for it, whatever slipped past the parser, so those tests build the combination without the check.
    fn meta_from_unchecked(extra: &str) -> Meta {
        let d = format!("t\n* bench.id: x\n* bench.group: accuracy\n{extra}\n.op\n.end\n");
        parse(&d, Some("x")).meta
    }

    fn plot(name: &str, vars: &[(&str, Vec<f64>)]) -> Plot {
        Plot {
            title: "t".into(),
            plotname: name.into(),
            vars: vars
                .iter()
                .map(|(n, _)| Var {
                    name: n.to_string(),
                    kind: "voltage".into(),
                })
                .collect(),
            npoints: vars[0].1.len(),
            cols: vars.iter().map(|(_, c)| c.clone()).collect(),
        }
    }

    fn op(v: &[(&str, f64)]) -> Vec<Plot> {
        vec![plot(
            "Operating Point",
            &v.iter().map(|(n, x)| (*n, vec![*x])).collect::<Vec<_>>(),
        )]
    }

    fn run(m: &Meta, deck: &str, rs: &[Plot], rf: &[Plot]) -> CompareResult {
        compare(&CompareInput {
            meta: m,
            deck,
            rs,
            reference: rf,
            asis: None,
        })
    }

    #[test]
    fn op_pass_and_fail_with_abs_rel_tolerance() {
        let m = meta_from("* bench.tol.rel: 1e-6\n* bench.tol.abs: 1e-9");
        let deck = "t\n.op\n";
        let rf = op(&[("v(a)", 2.5), ("v(b)", 1.0)]);
        let good = op(&[("v(a)", 2.5 + 1e-8), ("v(b)", 1.0)]); // 1e-8 <= 1e-9 + 2.5e-6
        let r = run(&m, deck, &good, &rf);
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        let bad = op(&[("v(a)", 2.5 + 1e-5), ("v(b)", 1.0)]);
        let r = run(&m, deck, &bad, &rf);
        assert_eq!(r.pass, Some(false));
        assert!(r.failures[0].contains("v(a)"));
        let pa = r.probes.iter().find(|p| p.name == "v(a)").unwrap();
        assert!((pa.abs_err.unwrap() - 1e-5).abs() < 1e-12);
        assert_eq!(pa.pass, Some(false));
    }

    #[test]
    fn vacuous_reference_is_never_a_pass() {
        let m = meta_from("* bench.floor: 1e-6");
        let deck = "t\n.op\n";
        let rf = op(&[("v(a)", 1e-34)]);
        let rs = op(&[("v(a)", 3e-34)]);
        let r = run(&m, deck, &rs, &rf);
        assert!(r.probes[0].vacuous);
        assert_eq!(r.pass, None, "all-vacuous must not pass");
        // but a gross mismatch against a ~0 reference still fails
        let rs = op(&[("v(a)", 1e-3)]);
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(false));
    }

    #[test]
    fn missing_probe_handling() {
        let m = meta_from("* bench.probes: v(a), v(zz)");
        let deck = "t\n.op\n";
        let rf = op(&[("v(a)", 1.0), ("v(zz)", 2.0)]);
        let rs = op(&[("v(a)", 1.0)]);
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(false)); // rustspice lacks the probe
        let rf2 = op(&[("v(a)", 1.0)]);
        let r = run(&m, deck, &rs, &rf2);
        assert_eq!(r.pass, None); // cannot be verified, never silently passes
    }

    #[test]
    fn non_finite_rustspice_value_fails() {
        let m = meta_from("");
        let r = run(
            &m,
            "t\n.op\n",
            &op(&[("v(a)", f64::NAN)]),
            &op(&[("v(a)", 1.0)]),
        );
        assert_eq!(r.pass, Some(false));
    }

    #[test]
    fn within_ng_error_flag() {
        let m = meta_from("* bench.tol.rel: 1e-9\n* bench.tol.abs: 1e-12");
        let rf = op(&[("v(a)", 1.0)]);
        let rs = op(&[("v(a)", 1.0 + 2e-5)]);
        let ng = op(&[("v(a)", 1.0 + 5e-5)]);
        let r = compare(&CompareInput {
            meta: &m,
            deck: "t\n.op\n",
            rs: &rs,
            reference: &rf,
            asis: Some(&ng),
        });
        assert_eq!(r.pass, Some(false));
        assert_eq!(r.probes[0].within_ng_error, Some(true));
        assert_eq!(r.within_ng_error, Some(true));
    }

    #[test]
    fn dc_alignment_and_point_count() {
        let m = meta_from("* bench.tol.rel: 1e-6\n* bench.tol.abs: 1e-9");
        let deck = "t\n.dc v1 0 2 1\n";
        let mk = |ax: Vec<f64>, y: Vec<f64>| {
            vec![plot(
                "DC transfer characteristic",
                &[("v(v-sweep)", ax), ("v(a)", y)],
            )]
        };
        let rf = mk(vec![0.0, 1.0, 2.0], vec![0.0, 0.5, 1.0]);
        let r = run(&m, deck, &mk(vec![0.0, 1.0, 2.0], vec![0.0, 0.5, 1.0]), &rf);
        assert_eq!(r.pass, Some(true), "{:?} {:?}", r.failures, r.notes);
        assert_eq!(r.probes.len(), 1); // sweep axis excluded from auto probes
        let r = run(&m, deck, &mk(vec![0.0, 1.0], vec![0.0, 0.5]), &rf);
        assert_eq!(r.pass, Some(false));
        assert!(r.failures[0].contains("point-count"));
        let r = run(
            &m,
            deck,
            &mk(vec![0.0, 1.0, 2.0], vec![0.0, 0.5, 1.001]),
            &rf,
        );
        assert_eq!(r.pass, Some(false));
        assert!((r.probes[0].worst_at.unwrap() - 2.0).abs() < 1e-12);
        // 1e-10 relative grid mismatch is tolerated
        let r = run(
            &m,
            deck,
            &mk(vec![0.0, 1.0 + 1e-10, 2.0], vec![0.0, 0.5, 1.0]),
            &rf,
        );
        assert_eq!(r.pass, Some(true));
    }

    fn tran_plots(f: impl Fn(f64) -> f64, tstop: f64, n: usize) -> Vec<Plot> {
        let t: Vec<f64> = (0..=n).map(|k| tstop * k as f64 / n as f64).collect();
        let y: Vec<f64> = t.iter().map(|&x| f(x)).collect();
        vec![plot("Transient Analysis", &[("time", t), ("v(a)", y)])]
    }

    #[test]
    fn tran_norm_error_and_resampling() {
        let m = meta_from("* bench.tol.norm: 5e-3\n* bench.tags: timing_edge");
        let deck = "t\n.tran 1u 1m\n";
        let f = |t: f64| (2.0 * std::f64::consts::PI * 1e3 * t).sin();
        let rf = tran_plots(f, 1e-3, 20000);
        // coarser grid than the reference: still within tolerance after resampling
        let rs = tran_plots(f, 1e-3, 1000);
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        assert!(r.probes[0].norm_err.unwrap() < 5e-3);
        assert!(r.interp_floor.unwrap() < 1e-4);
        // 2 % amplitude error fails
        let rs = tran_plots(|t| 1.02 * f(t), 1e-3, 1000);
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(false));
        let n = r.probes[0].norm_err.unwrap();
        assert!((n - 0.02 / 2.0).abs() < 2e-3, "norm_err {n}"); // swing 2 -> 0.02/2
                                                                // edge shift: a pulse delayed by 5 us moves both 50 % crossings by 5 us
        let pulse = |t: f64| ((t - 3e-4) / 2e-5).tanh() - ((t - 7e-4) / 2e-5).tanh();
        let rf2 = tran_plots(pulse, 1e-3, 20000);
        let rs = tran_plots(|t| pulse(t - 5e-6), 1e-3, 1000);
        let r = run(&m, deck, &rs, &rf2);
        let shift = r.probes[0].edge_shift.unwrap();
        assert!((shift - 5e-6).abs() < 1e-7, "shift {shift}");
        assert!((r.probes[0].edge_shift_at_rows.unwrap() - 5e-6).abs() < 1e-7);
        // exact values on rows far coarser than the edge: the whole shift is the rows' resolution, not the simulator
        let sharp = |t: f64| ((t - 3.03e-4) / 2e-6).tanh() - ((t - 7.03e-4) / 2e-6).tanh();
        let rf3 = tran_plots(sharp, 1e-3, 100_000);
        let rs = tran_plots(sharp, 1e-3, 100);
        let r = run(&m, deck, &rs, &rf3);
        let (naive, at_rows, floor) = (
            r.probes[0].edge_shift.unwrap(),
            r.probes[0].edge_shift_at_rows.unwrap(),
            r.probes[0].edge_row_floor.unwrap(),
        );
        assert!(naive > 1e-6, "{naive}");
        assert!(
            at_rows < 1e-4 * naive && (floor - naive).abs() < 1e-4 * naive,
            "{naive} {at_rows} {floor}"
        );
        // rustspice stops early
        let rs = tran_plots(f, 5e-4, 500);
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(false));
        assert!(r.failures.iter().any(|x| x.contains("ends at")));
    }

    #[test]
    fn crossing_detection() {
        let t = [0.0, 1.0, 2.0, 3.0];
        let y = [0.0, 1.0, 0.0, 1.0];
        let c = crossings(&t, &y, 0.5);
        assert_eq!(c, vec![0.5, 1.5, 2.5]);
        assert_eq!(interp(&t, &y, 0.25), 0.25);
        assert_eq!(interp(&t, &y, -1.0), 0.0);
        assert_eq!(interp(&t, &y, 9.0), 1.0);
    }

    // ---- classification: every status and the known_gap transitions -------------------------------------

    fn ok() -> NgSide {
        NgSide::Ok
    }

    #[test]
    fn classification_table() {
        let m = meta_from("");
        let c = |ng: NgSide, rs: RsSide, pass: Option<bool>| classify(&m, &ng, &rs, pass);
        assert_eq!(c(ok(), RsSide::Ok, Some(true)).status, Status::Pass);
        assert_eq!(
            c(ok(), RsSide::Ok, Some(false)).status,
            Status::FailAccuracy
        );
        assert_eq!(c(ok(), RsSide::Ok, None).status, Status::Unverified);
        assert_eq!(
            c(ok(), RsSide::LoudError("x".into()), None).status,
            Status::FailRobustness
        );
        assert_eq!(
            c(NgSide::Error("x".into()), RsSide::Ok, None).status,
            Status::NgspiceError
        );
        assert_eq!(
            c(NgSide::Timeout, RsSide::Ok, None).status,
            Status::NgspiceError
        );
        assert_eq!(
            c(
                NgSide::Error("x".into()),
                RsSide::LoudError("y".into()),
                None
            )
            .status,
            Status::BothError
        );
        assert_eq!(
            c(ok(), RsSide::Panic("p".into()), None).status,
            Status::Panic
        );
        assert_eq!(c(ok(), RsSide::Timeout, None).status, Status::Timeout);
        assert_eq!(
            c(NgSide::Missing, RsSide::Ok, Some(true)).status,
            Status::Unverified
        );
        // outcomes are independent of status
        assert_eq!(
            c(ok(), RsSide::Ok, Some(false)).outcome,
            Some(Outcome::Mismatch)
        );
        assert_eq!(
            c(NgSide::Error("x".into()), RsSide::Ok, None).outcome,
            Some(Outcome::NgspiceOnlyError)
        );
        assert_eq!(
            c(ok(), RsSide::LoudError("y".into()), None).outcome,
            Some(Outcome::LoudError)
        );
    }

    #[test]
    fn expect_loud_error_and_either() {
        let le = meta_from("* bench.expect: loud_error");
        assert_eq!(
            classify(&le, &ok(), &RsSide::LoudError("no".into()), None).status,
            Status::Pass
        );
        let v = classify(&le, &ok(), &RsSide::Ok, Some(true));
        assert_eq!(v.status, Status::FailSilent);
        assert_eq!(v.outcome, Some(Outcome::SilentNumbers));
        // loud_error + ngspice also refusing is still a pass for rustspice
        assert_eq!(
            classify(
                &le,
                &NgSide::Error("e".into()),
                &RsSide::LoudError("no".into()),
                None
            )
            .status,
            Status::Pass
        );
        let ei = meta_from("* bench.expect: either");
        assert_eq!(
            classify(&ei, &ok(), &RsSide::Ok, Some(false)).status,
            Status::Info
        );
        assert_eq!(
            classify(&ei, &ok(), &RsSide::LoudError("x".into()), None).status,
            Status::Info
        );
        // a panic is always a violation, even for either
        assert_eq!(
            classify(&ei, &ok(), &RsSide::Panic("p".into()), None).status,
            Status::Panic
        );
        assert_eq!(
            classify(&ei, &NgSide::Error("x".into()), &RsSide::Timeout, None).status,
            Status::Timeout
        );
    }

    #[test]
    fn known_gap_transitions() {
        let kg = meta_from("* bench.known_gap: clamp-rail\n* bench.gap.note: test gap");
        // still violating -> confirmed (any violation kind except a crash)
        assert_eq!(
            classify(&kg, &ok(), &RsSide::LoudError("conv".into()), None).status,
            Status::KnownGapConfirmed
        );
        assert_eq!(
            classify(&kg, &ok(), &RsSide::Ok, Some(false)).status,
            Status::KnownGapConfirmed
        );
        // a panic or timeout is covered only by a crash slug (see `known_gap_covers_panic_and_timeout_only_for_crash_slugs`)
        assert_eq!(
            classify(&kg, &ok(), &RsSide::Panic("p".into()), None).status,
            Status::Panic
        );
        assert_eq!(
            classify(&kg, &ok(), &RsSide::Timeout, None).status,
            Status::Timeout
        );
        // fixed -> gap_closed
        assert_eq!(
            classify(&kg, &ok(), &RsSide::Ok, Some(true)).status,
            Status::GapClosed
        );
        // not judgeable without an ngspice verdict: keep the base status
        assert_eq!(
            classify(&kg, &NgSide::Error("x".into()), &RsSide::Ok, None).status,
            Status::NgspiceError
        );
        assert_eq!(
            classify(&kg, &NgSide::Missing, &RsSide::Ok, None).status,
            Status::Unverified
        );
        // loud_error + known gap
        let kl = meta_from("* bench.expect: loud_error\n* bench.known_gap: resource-budget\n* bench.gap.note: test gap");
        assert_eq!(
            classify(&kl, &ok(), &RsSide::Ok, None).status,
            Status::KnownGapConfirmed
        );
        assert_eq!(
            classify(&kl, &ok(), &RsSide::LoudError("refused".into()), None).status,
            Status::GapClosed
        );
        // without a known_gap the same violations are failures
        assert!(classify(&meta_from(""), &ok(), &RsSide::Ok, Some(false))
            .status
            .is_fail());
        assert!(!Status::KnownGapConfirmed.is_fail());
        assert!(!Status::GapClosed.is_fail());
    }

    #[test]
    fn status_strings_match_spec() {
        let names: Vec<&str> = Status::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "pass",
                "fail_accuracy",
                "fail_robustness",
                "fail_silent",
                "known_gap_confirmed",
                "gap_closed",
                "both_error",
                "ngspice_error",
                "panic",
                "timeout",
                "skipped_budget",
                "unverified",
                "info"
            ]
        );
        for s in Status::ALL {
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.as_str())
            );
        }
    }

    fn tran_at(ts: &[f64], f: impl Fn(f64) -> f64) -> Vec<Plot> {
        let y: Vec<f64> = ts.iter().map(|&x| f(x)).collect();
        vec![plot(
            "Transient Analysis",
            &[("time", ts.to_vec()), ("v(a)", y)],
        )]
    }

    fn steps(h: f64, n: usize) -> Vec<f64> {
        (0..=n).map(|k| k as f64 * h).collect()
    }

    #[test]
    fn tran_grid_follows_the_nominal_steps_and_the_window() {
        // 1e-4 / 5e-8 is not an exact integer in binary floating point: the grid must still be k * tstep
        let g = tran_grid(Some(5e-8), 0.0, 1e-4, &[], 200_000);
        assert_eq!(g.len(), 2001);
        assert_eq!(g[1], 5e-8);
        assert!((g[2000] - 1e-4).abs() < 1e-18);
        // partial last step: tstop is appended
        let g = tran_grid(Some(1e-6), 0.0, 10.5e-6, &[], 100);
        assert_eq!(g.len(), 12);
        assert_eq!(*g.last().unwrap(), 10.5e-6);
        // a window that starts off-grid (tstart) keeps its own first point
        let g = tran_grid(Some(10e-9), 2.005e-6, 5e-6, &[], 1000);
        assert_eq!(g[0], 2.005e-6);
        assert!((g[1] - 2.01e-6).abs() < 1e-15);
        // tstep >= window: the union of the given time rows
        let (ra, rb) = ([0.0, 1e-9], [0.0, 0.3e-9, 1e-9]);
        let g = tran_grid(Some(1e-6), 0.0, 1e-9, &[&ra, &rb], 100);
        assert_eq!(g, vec![0.0, 0.3e-9, 1e-9]);
        // unknown tstep behaves the same
        assert_eq!(tran_grid(None, 0.0, 1e-9, &[&ra, &rb], 100), g);
        // the cap thins the grid but keeps both ends
        let g = tran_grid(Some(1.0), 0.0, 1000.0, &[], 10);
        assert!(
            g.len() <= 12 && g[0] == 0.0 && *g.last().unwrap() == 1000.0,
            "{g:?}"
        );
        assert_eq!(expected_tran_rows(10e-9, 100e-6), 10001);
        assert_eq!(expected_tran_rows(1e-6, 10.5e-6), 12);
        assert_eq!(expected_tran_rows(1e-6, 1e-9), 2);
    }

    #[test]
    fn edge_narrower_than_tstep_is_compared_at_rustspice_sample_times() {
        // Regression (buck converter): a 20 ns edge on a 50 ns grid. rustspice samples the exact function at
        // k * 50 ns; the dense reference is the same function. Comparing at those instants is an exact match,
        // a grid with spacing tstop/ceil(tstop/tstep) is not (it samples the rustspice interpolant mid-edge).
        let f = |t: f64| 10.0 * ((t - 80e-6) / 20e-9).clamp(0.0, 1.0);
        let m = meta_from("* bench.tol.norm: 5e-3");
        let deck = "t\n.tran 50n 100u\n";
        let rf = tran_at(&steps(1e-9, 100_000), f);
        let rs = tran_at(&steps(50e-9, 2000), f);
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        assert!(r.probes[0].norm_err.unwrap() < 1e-9, "{:?}", r.probes[0]);
        assert!(
            r.notes.iter().all(|n| !n.contains("interpolation noise")),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn tstart_window_ignores_rustspice_rows_before_it() {
        let m = meta_from("");
        let deck = "t\n.tran 10n 5u 2u\n";
        let f = |t: f64| 1.0 - (-t / 1e-6).exp();
        let rf = tran_at(&steps(10e-9, 500)[200..], f); // ngspice: rows from 2 us
        let rs_t = steps(10e-9, 500);
        // rustspice rows before tstart are garbage: they must not matter
        let rs = tran_at(&rs_t, |t| if t < 2e-6 { 123.0 } else { f(t) });
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        assert!(
            r.notes.iter().any(|n| n.contains("tstart honoured")),
            "{:?}",
            r.notes
        );
        // an error inside the window is still found
        let bad = tran_at(&rs_t, |t| if t < 2e-6 { f(t) } else { f(t) + 0.3 });
        assert_eq!(run(&m, deck, &bad, &rf).pass, Some(false));
    }

    #[test]
    fn tstep_larger_than_tstop_compares_at_the_rustspice_rows() {
        // RC with tau = 1 ps and `.tran 1u 1n`: ngspice has dense internal rows, rustspice rows at 0 and 1 ns only.
        // Both agree at the two instants where rustspice has samples; interpolating its single step onto the
        // ngspice rows would report an error of ~1 that measures the interpolation, not the simulator.
        let m = meta_from("");
        let deck = "t\n.tran 1u 1n\n";
        let ts: Vec<f64> = (0..=200).map(|k| k as f64 * 5e-12).collect();
        let rf = tran_at(&ts, |t| 1.0 - (-t / 1e-12).exp());
        let good = tran_at(&[0.0, 1e-9], |t| 1.0 - (-t / 1e-12).exp());
        let r = run(&m, deck, &good, &rf);
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        assert!(
            r.notes
                .iter()
                .any(|n| n.contains("at the rustspice time rows")),
            "{:?}",
            r.notes
        );
        // a wrong end value (a backward-Euler step over the whole window) is visible
        let bad = tran_at(&[0.0, 1e-9], |t| 0.5 * (1.0 - (-t / 1e-12).exp()));
        assert_eq!(run(&m, deck, &bad, &rf).pass, Some(false));
    }

    #[test]
    fn parameterised_tran_card_takes_its_window_from_the_ngspice_rows() {
        let m = meta_from("");
        let deck = "t\n.param n=5\n.tran 10n {1u*n}\n";
        let rf = tran_at(&steps(10e-9, 500), |t| t);
        let rs = tran_at(&steps(10e-9, 500), |t| t);
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        assert!(r.notes.iter().any(|n| n.contains("not a plain number")));
        // rustspice stopping early (the truncated `{1u*n}` => 1 us) is a failure against the ngspice window
        let short = tran_at(&steps(10e-9, 100), |t| t);
        let r = run(&m, deck, &short, &rf);
        assert_eq!(r.pass, Some(false));
        assert!(r.failures.iter().any(|x| x.contains("ends at")));
    }

    #[test]
    fn npoints_tag_checks_the_row_count() {
        let m = meta_from("* bench.tags: npoints");
        let deck = "t\n.tran 1u 10.5u\n";
        let f = |t: f64| t;
        let rf = tran_at(&steps(0.1e-6, 105), f);
        let mut t12 = steps(1e-6, 10);
        t12.push(10.5e-6);
        let ok = tran_at(&t12, f);
        let r = run(&m, deck, &ok, &rf);
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        // one extra sliver row (the audit finding): 13 rows
        let mut t13 = t12.clone();
        t13.push(10.5e-6 + 1e-17);
        let r = run(&m, deck, &tran_at(&t13, f), &rf);
        assert_eq!(r.pass, Some(false));
        assert!(
            r.failures.iter().any(|x| x.contains("expected 12")),
            "{:?}",
            r.failures
        );
        // ... unless ngspice as-is writes the same number of rows
        let r = compare_with(
            &CompareInput {
                meta: &m,
                deck,
                rs: &tran_at(&t13, f),
                reference: &rf,
                asis: None,
            },
            Some(13),
        );
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        // without the tag the row count is not judged
        let m0 = meta_from("");
        assert_eq!(run(&m0, deck, &tran_at(&t13, f), &rf).pass, Some(true));
    }

    #[test]
    fn skipped_ngspice_is_judged_on_expect_only() {
        let sk = NgSide::Skipped;
        let le = meta_from("* bench.expect: loud_error");
        assert_eq!(
            classify(&le, &sk, &RsSide::LoudError("refused".into()), None).status,
            Status::Pass
        );
        assert_eq!(
            classify(&le, &sk, &RsSide::Ok, None).status,
            Status::FailSilent
        );
        let ei = meta_from("* bench.expect: either\n* bench.known_gap: resource-budget\n* bench.gap.note: test gap");
        // a panic / abort stays a violation: known gap confirmed
        assert_eq!(
            classify(&ei, &sk, &RsSide::Panic("SIGABRT".into()), None).status,
            Status::KnownGapConfirmed
        );
        assert_eq!(
            classify(&ei, &sk, &RsSide::Timeout, None).status,
            Status::KnownGapConfirmed
        );
        // a loud refusal closes it
        assert_eq!(
            classify(&ei, &sk, &RsSide::LoudError("budget".into()), None).status,
            Status::GapClosed
        );
        // without a known gap the same panic is a failure
        let ei0 = meta_from("* bench.expect: either");
        assert_eq!(
            classify(&ei0, &sk, &RsSide::Panic("p".into()), None).status,
            Status::Panic
        );
        assert_eq!(
            classify(&ei0, &sk, &RsSide::Ok, None).outcome,
            None,
            "no reference: the outcome is not judged"
        );
        // expect=match has nothing to compare with: unverified, never a pass
        let ma = meta_from("");
        assert_eq!(
            classify(&ma, &sk, &RsSide::Ok, None).status,
            Status::Unverified
        );
        assert_eq!(
            classify(&ma, &sk, &RsSide::LoudError("x".into()), None).status,
            Status::Unverified
        );
    }

    #[test]
    fn unevaluable_comparison_under_either_has_no_outcome() {
        let ei = meta_from("* bench.expect: either");
        let v = classify(&ei, &NgSide::Ok, &RsSide::Ok, None);
        assert_eq!(v.status, Status::Info);
        assert_eq!(v.outcome, None);
    }

    // ---- SUITE_SPEC 14.3: the resampling-floor rule ----------------------------------------------------

    fn sine(t: f64) -> f64 {
        (2.0 * std::f64::consts::PI * 1e3 * t).sin()
    }

    fn floor_case(tol: &str, rs_gain: f64, ref_rows: usize) -> CompareResult {
        let m = meta_from(&format!("* bench.tol.norm: {tol}"));
        let rf = tran_plots(sine, 1e-3, ref_rows);
        let rs = tran_plots(|t| rs_gain * sine(t), 1e-3, 1000);
        run(&m, "t\n.tran 1u 1m\n", &rs, &rf)
    }

    #[test]
    fn a_pass_never_carries_the_resampling_floor_note() {
        // dense reference: the floor is far below the tolerance, a plain pass, no remark
        let r = floor_case("5e-3", 1.0, 20000);
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        assert!(
            !r.notes.iter().any(|n| n.contains("resampling floor")),
            "{:?}",
            r.notes
        );
        assert!(r.interp_floor.unwrap() < 5e-3);
        // a coarse reference (40 rows per period) has a floor above the tolerance
        let coarse = floor_case("2e-3", 1.0, 40);
        let fl = coarse.probes[0].interp_floor.unwrap();
        assert!(fl > 2e-3, "test setup: floor {fl}");
        // the error (~1.5e-3 of linear interpolation) is within the floor of the limit: not a pass, not a fail
        let e = coarse.probes[0].norm_err.unwrap();
        assert!((e - 2e-3).abs() < fl, "test setup: err {e}");
        assert_eq!(coarse.pass, None, "{:?}", coarse.failures);
        assert_eq!(coarse.probes[0].pass, None);
        assert!(coarse.failures.is_empty());
        assert!(
            coarse
                .notes
                .iter()
                .any(|n| n.contains("resampling floor") && n.contains("cannot decide")),
            "{:?}",
            coarse.notes
        );
    }

    #[test]
    fn a_corner_on_a_grid_point_with_its_own_reference_row_adds_no_floor() {
        // reference: 0 until a 10 ns edge that starts exactly on the 1 us grid point t = 500 us, rows every 0.3 us
        // plus rows at the two corners (as a simulator that lands on its breakpoints writes them)
        let edge = |t: f64| ((t - 500e-6) / 10e-9).clamp(0.0, 1.0);
        let mut t: Vec<f64> = (0..=3333).map(|k| k as f64 * 0.3e-6).collect();
        t.extend([500e-6, 500.01e-6]);
        t.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let y: Vec<f64> = t.iter().map(|&x| edge(x)).collect();
        let grid: Vec<f64> = (0..=1000).map(|k| k as f64 * 1e-6).collect();
        let fl = decimation_floor(&t, &y, &grid, 1.0);
        assert!(fl < 1e-9, "floor {fl}");
        // without the row at the corner the comparison itself interpolates there: the floor shows it
        let keep: Vec<usize> = (0..t.len()).filter(|&k| t[k] != 500e-6).collect();
        let (t2, y2): (Vec<f64>, Vec<f64>) = keep.iter().map(|&k| (t[k], y[k])).unzip();
        assert!(decimation_floor(&t2, &y2, &grid, 1.0) > 1e-3);
    }

    #[test]
    fn a_fail_inside_the_noise_is_unverified_and_a_clear_fail_has_no_floor_note() {
        // error just above the tolerance but inside the floor: the verdict would depend on the resampling
        let near = floor_case("1e-3", 1.0, 40);
        let (e, fl) = (
            near.probes[0].norm_err.unwrap(),
            near.probes[0].interp_floor.unwrap(),
        );
        assert!(
            e > 1e-3 && fl > 1e-3 && (e - 1e-3).abs() < fl,
            "setup: err {e} floor {fl}"
        );
        assert_eq!(
            near.pass, None,
            "an error within the noise of the limit is not a fail"
        );
        // a 20 % amplitude error is far above tolerance + floor: a plain fail without the remark
        let clear = floor_case("2e-3", 1.2, 40);
        let e = clear.probes[0].norm_err.unwrap();
        let fl = clear.probes[0].interp_floor.unwrap();
        assert!(e >= 2e-3 + fl, "setup: err {e} floor {fl}");
        assert_eq!(clear.pass, Some(false));
        assert!(clear.failures.iter().any(|f| f.contains("norm_err")));
        assert!(
            !clear.notes.iter().any(|n| n.contains("resampling floor")),
            "the remark must not contradict a fail: {:?}",
            clear.notes
        );
    }

    #[test]
    fn probes_report_how_close_they_are_to_the_limit() {
        let m = meta_from("* bench.tol.rel: 1e-6\n* bench.tol.abs: 1e-9");
        let rf = op(&[("v(a)", 2.0)]);
        let rs = op(&[("v(a)", 2.0 + 1.0e-6)]);
        let r = run(&m, "t\n.op\n", &rs, &rf);
        // allowed = 1e-9 + 2e-6; error 1e-6 -> ratio ~0.5
        let ratio = r.probes[0].tol_ratio.unwrap();
        assert!((ratio - 1e-6 / (1e-9 + 2e-6)).abs() < 1e-9, "{ratio}");
        // vacuous probes carry no ratio
        let rf = op(&[("v(a)", 1e-34)]);
        let r = run(&m, "t\n.op\n", &op(&[("v(a)", 2e-34)]), &rf);
        assert_eq!(r.probes[0].tol_ratio, None);
    }

    // ---- E3a hardening: every test below fails on the pre-hardening engine ---------------------------------

    fn rc(t: f64) -> f64 {
        1.0 - (-t / 1e-6).exp()
    }

    /// ER-metrics-01: a non-finite reference or time axis must never be a pass.
    #[test]
    fn non_finite_reference_is_unverified_and_non_finite_rustspice_time_fails() {
        let m = meta_from("");
        let deck = "t\n.tran 1u 1m\n";
        let n = 1000;
        let t: Vec<f64> = (0..=n).map(|k| 1e-3 * k as f64 / n as f64).collect();
        let good = tran_at(&t, sine);
        // (a) the reference is NaN everywhere: nothing can be judged, but nothing may pass
        let r = run(&m, deck, &good, &tran_at(&t, |_| f64::NAN));
        assert_eq!(r.pass, None, "all-NaN reference: {:?}", r.probes);
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        assert_eq!(r.probes[0].pass, None);
        assert!(
            r.probes[0].norm_err.is_none() && r.probes[0].abs_err.is_none(),
            "an unevaluated error is null, not 0: {:?}",
            r.probes[0]
        );
        assert!(
            r.notes.iter().any(|x| x.contains("not finite")),
            "{:?}",
            r.notes
        );
        // (b) half the reference is NaN and the finite half agrees: the unchecked half is never a pass
        let half = tran_at(&t, |x| if x < 5e-4 { sine(x) } else { f64::NAN });
        let r = run(&m, deck, &good, &half);
        assert_ne!(
            r.pass,
            Some(true),
            "half-NaN reference passed: {:?}",
            r.notes
        );
        assert!(
            r.notes.iter().any(|x| x.contains("not finite")),
            "{:?}",
            r.notes
        );
        // ... but a violation inside the finite half is still a failure
        let bad = tran_at(&t, |x| sine(x) + 0.3);
        assert_eq!(run(&m, deck, &bad, &half).pass, Some(false));
        // (c) the RustSpice time axis turns NaN while the values stay finite garbage
        let mut tn = t.clone();
        for x in tn.iter_mut().skip(500) {
            *x = f64::NAN;
        }
        let garbage = vec![plot(
            "Transient Analysis",
            &[
                ("time", tn),
                (
                    "v(a)",
                    (0..=n)
                        .map(|k| if k < 500 { sine(t[k]) } else { 9.0 })
                        .collect(),
                ),
            ],
        )];
        let r = run(&m, deck, &garbage, &tran_at(&t, sine));
        assert_eq!(r.pass, Some(false), "NaN time axis passed: {:?}", r.probes);
        assert!(
            r.failures.iter().any(|x| x.contains("time")),
            "{:?}",
            r.failures
        );
        // (d) only the last RustSpice time stamp is NaN
        let mut tl = t.clone();
        *tl.last_mut().unwrap() = f64::NAN;
        let r = run(&m, deck, &tran_at(&tl, sine), &tran_at(&t, sine));
        assert_eq!(r.pass, Some(false), "NaN last time stamp passed");
    }

    /// ER-metrics-04: non-finite reference points of a dc sweep are counted and reported, never silently skipped.
    #[test]
    fn dc_non_finite_reference_points_are_counted() {
        let m = meta_from("* bench.tol.rel: 1e-6\n* bench.tol.abs: 1e-9");
        let deck = "t\n.dc v1 0 3 1\n";
        let mk = |y: Vec<f64>| {
            vec![plot(
                "DC transfer characteristic",
                &[("v(v-sweep)", vec![0.0, 1.0, 2.0, 3.0]), ("v(a)", y)],
            )]
        };
        let rf = mk(vec![0.0, 1.0, f64::NAN, f64::NAN]);
        let r = run(&m, deck, &mk(vec![0.0, 1.0, 9.0, 9.0]), &rf);
        assert_eq!(r.pass, None, "{:?}", r.probes);
        assert!(
            r.notes
                .iter()
                .any(|x| x.contains("2 of 4") && x.contains("not finite")),
            "{:?}",
            r.notes
        );
        // a violation on the finite points is still a fail
        let r = run(&m, deck, &mk(vec![0.0, 1.5, 9.0, 9.0]), &rf);
        assert_eq!(r.pass, Some(false));
        // an all-finite reference is unaffected
        let r = run(
            &m,
            deck,
            &mk(vec![0.0, 1.0, 2.0, 3.0]),
            &mk(vec![0.0, 1.0, 2.0, 3.0]),
        );
        assert_eq!(r.pass, Some(true));
    }

    /// ER-metrics-05: a reference transient that is too short is the reference's problem.
    #[test]
    fn broken_reference_transient_is_unverified_not_a_rustspice_failure() {
        let m = meta_from("");
        let deck = "t\n.tran 1u 1m\n";
        let good = tran_plots(sine, 1e-3, 1000);
        for t in [vec![0.0], vec![0.0, 0.0], vec![2e-4, 2e-4]] {
            let r = run(&m, deck, &good, &tran_at(&t, sine));
            assert_eq!(r.pass, None, "reference rows {t:?}: {:?}", r.failures);
            assert!(r.failures.is_empty(), "{t:?}: {:?}", r.failures);
            assert!(
                r.notes.iter().any(|x| x.contains("unverified")),
                "{:?}",
                r.notes
            );
        }
        // a reference that stops early but has rows is still compared on the common window (SUITE_SPEC 13.3)
        let early = tran_at(
            &(0..=100).map(|k| 1e-6 * k as f64).collect::<Vec<_>>(),
            sine,
        );
        assert_eq!(run(&m, deck, &good, &early).pass, Some(true));
        // a RustSpice run that stops early is still a failure, whatever the reference
        let short = tran_plots(sine, 5e-4, 500);
        assert_eq!(run(&m, deck, &short, &good).pass, Some(false));
    }

    /// ER-metrics-02 / ER-driver-04: a known_gap covers a panic or timeout only when its slug is a crash slug.
    #[test]
    fn known_gap_covers_panic_and_timeout_only_for_crash_slugs() {
        for slug in [
            "clamp-rail",
            "silent-ignore-directive",
            "unsupported-syntax-loud",
        ] {
            let m = meta_from(&format!(
                "* bench.known_gap: {slug}\n* bench.gap.note: test gap"
            ));
            let v = classify(&m, &ok(), &RsSide::Panic("p".into()), None);
            assert_eq!(v.status, Status::Panic, "{slug}");
            assert!(v.status.is_fail());
            assert_eq!(v.outcome, Some(Outcome::Panic));
            let v = classify(&m, &ok(), &RsSide::Timeout, None);
            assert_eq!(v.status, Status::Timeout, "{slug}");
            // the other violation classes stay covered by any slug
            assert_eq!(
                classify(&m, &ok(), &RsSide::Ok, Some(false)).status,
                Status::KnownGapConfirmed
            );
        }
        for slug in ["resource-budget", "symbolic-lu-blowup"] {
            let m = meta_from(&format!(
                "* bench.known_gap: {slug}\n* bench.gap.note: test gap"
            ));
            assert_eq!(
                classify(&m, &ok(), &RsSide::Panic("p".into()), None).status,
                Status::KnownGapConfirmed
            );
            assert_eq!(
                classify(&m, &ok(), &RsSide::Timeout, None).status,
                Status::KnownGapConfirmed
            );
        }
    }

    /// ER-metrics-03: the unit-blind default floor must not hide a current that RustSpice gets wrong.
    #[test]
    fn default_floor_does_not_hide_a_deviating_current() {
        let m = meta_from("* bench.probes: v(out), i(v1)");
        let deck = "t\n.op\n";
        let rf = op(&[("v(out)", 0.5), ("i(v1)", -5e-8)]);
        let rs = op(&[("v(out)", 0.5), ("i(v1)", -1.5e-7)]); // +200 %, 1e-7 A > tol.abs
        let r = run(&m, deck, &rs, &rf);
        let pi = r.probes.iter().find(|p| p.name == "i(v1)").unwrap();
        assert_eq!(pi.pass, None);
        assert!(!pi.vacuous, "a deviating probe is unverified, not vacuous");
        let note = pi.note.as_deref().unwrap_or("");
        assert!(
            note.contains(" A") && note.contains("floor") && note.contains("bench.floor"),
            "{note}"
        );
        assert_eq!(r.pass, None, "the case must not silently pass");
        // the deck set bench.floor: the author decided, the probe is vacuous as before
        let r = run(&m, "t\n* bench.floor: 1e-6\n.op\n", &rs, &rf);
        assert_eq!(r.pass, Some(true), "{:?}", r.probes);
        // a deviation within tol.abs stays a plain vacuous probe
        let near = op(&[("v(out)", 0.5), ("i(v1)", -5e-8 + 1e-10)]);
        let r = run(&m, deck, &near, &rf);
        assert_eq!(r.pass, Some(true), "{:?}", r.probes);
        assert!(r.probes.iter().find(|p| p.name == "i(v1)").unwrap().vacuous);
        // the same on a transient
        let tr = |amp: f64| tran_plots(move |t| amp * sine(t), 1e-3, 1000);
        let m = meta_from("");
        let r = run(&m, "t\n.tran 1u 1m\n", &tr(1.5e-7), &tr(5e-8));
        assert_eq!(r.pass, None, "{:?}", r.probes);
        assert!(!r.probes[0].vacuous && !r.vacuous_only);
        assert!(r.probes[0]
            .note
            .as_deref()
            .unwrap_or("")
            .contains("bench.floor"));
        // explicit floor: vacuous as before (nothing evaluated, nothing wrong)
        let r = run(
            &m,
            "t\n* bench.floor: 1e-6\n.tran 1u 1m\n",
            &tr(1.5e-7),
            &tr(5e-8),
        );
        assert!(r.probes[0].vacuous && r.vacuous_only, "{:?}", r.probes);
    }

    /// CAL (b): several analyses of one kind are paired by span, not by "the last one of its kind".
    #[test]
    fn plots_of_the_same_kind_are_paired_by_their_span() {
        let m = meta_from("");
        let deck = "t\n.tran 10n 2u\n.tran 10n 5u\n";
        let p2 = tran_at(&steps(10e-9, 200), rc);
        let p5 = tran_at(&steps(10e-9, 500), rc);
        let cat = |a: &[Plot], b: &[Plot]| a.iter().chain(b).cloned().collect::<Vec<Plot>>();
        // RustSpice: card order; ngspice: reverse card order
        let rs = cat(&p2, &p5);
        let rf = cat(&p5, &p2);
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(true), "{:?} {:?}", r.failures, r.notes);
        assert_eq!(r.probes.len(), 2, "both plots are compared: {:?}", r.probes);
        assert!(
            r.notes.iter().any(|x| x.contains("paired by")),
            "{:?}",
            r.notes
        );
        // an error confined to the FIRST rustspice plot is found (the last-of-kind rule never looked at it)
        let bad2 = tran_at(&steps(10e-9, 200), |t| rc(t) + 0.2);
        let r = run(&m, deck, &cat(&bad2, &p5), &rf);
        assert_eq!(r.pass, Some(false), "{:?}", r.probes);
        // the same with the reference in card order (mutant runs compare two RustSpice outputs)
        let r = run(&m, deck, &cat(&bad2, &p5), &cat(&p2, &p5));
        assert_eq!(r.pass, Some(false));
        let r = run(&m, deck, &rs, &cat(&p2, &p5));
        assert_eq!(r.pass, Some(true), "{:?}", r.failures);
        // rustspice wrote fewer plots than the reference has
        let r = run(&m, deck, &p5, &rf);
        assert_eq!(r.pass, Some(false), "a dropped analysis must not pass");
        // two .op plots with different values: paired by value, in any order
        let o = |v: f64| plot("Operating Point", &[("v(a)", vec![v])]);
        let r = run(&m, "t\n.op\n.op\n", &[o(1.0), o(2.0)], &[o(2.0), o(1.0)]);
        assert_eq!(r.pass, Some(true), "{:?}", r.probes);
        let r = run(&m, "t\n.op\n.op\n", &[o(1.0), o(2.5)], &[o(2.0), o(1.0)]);
        assert_eq!(r.pass, Some(false));
        // two .dc sweeps with different spans
        let d = |last: f64, n: usize| {
            let ax: Vec<f64> = (0..n).map(|k| last * k as f64 / (n - 1) as f64).collect();
            let y: Vec<f64> = ax.iter().map(|x| 0.5 * x).collect();
            plot(
                "DC transfer characteristic",
                &[("v(v-sweep)", ax), ("v(a)", y)],
            )
        };
        let (d1, d4) = (d(1.0, 3), d(4.0, 5));
        let r = run(
            &m,
            "t\n.dc v1 0 1 0.5\n.dc v1 0 4 1\n",
            &[d1.clone(), d4.clone()],
            &[d4, d1],
        );
        assert_eq!(r.pass, Some(true), "{:?} {:?}", r.failures, r.probes);
        assert_eq!(r.probes.len(), 2);
    }

    /// ER-metrics-09: the grid is bounded for any step ratio (the u64 casts saturate and the count overflowed).
    #[test]
    fn tran_grid_is_bounded_for_extreme_step_ratios() {
        for (h, t0, t1) in [
            (1e-21, 0.0, 1.0),
            (1e-9, 5e18, 6e18),
            (1e-300, 0.0, 1e300),
            (f64::MIN_POSITIVE, 0.0, 1.0),
            (1e-9, 0.0, 1e12),
        ] {
            let g = tran_grid(Some(h), t0, t1, &[], 200_000);
            assert!(g.len() <= 200_004, "h={h:e}: {} points", g.len());
            assert!(g.len() >= 2 && g.iter().all(|x| x.is_finite()), "h={h:e}");
            assert!(g.windows(2).all(|w| w[0] <= w[1]), "h={h:e}: not sorted");
            assert_eq!((g[0], *g.last().unwrap()), (t0, t1), "h={h:e}");
        }
    }

    /// CAL (a): a dc grid mismatch still reports the errors over the aligned leading points as evidence, without
    /// changing the verdict.
    #[test]
    fn dc_grid_mismatch_reports_aligned_prefix_evidence_and_keeps_the_verdict() {
        let m = meta_from("* bench.tol.rel: 1e-6\n* bench.tol.abs: 1e-9");
        let deck = "t\n.dc v1 0 5 1\n";
        let mk = |ax: Vec<f64>| {
            let y: Vec<f64> = ax.iter().map(|x| 0.5 * x).collect();
            vec![plot(
                "DC transfer characteristic",
                &[("v(v-sweep)", ax), ("v(a)", y)],
            )]
        };
        // RustSpice drops the last point of the sweep (grid drift) and its values drift by 1e-12
        let rf = mk((0..=5).map(|k| k as f64).collect());
        let rs: Vec<Plot> = {
            let mut p = mk((0..5).map(|k| k as f64).collect());
            for v in p[0].cols[1].iter_mut() {
                *v += 1e-12;
            }
            p
        };
        let r = run(&m, deck, &rs, &rf);
        assert_eq!(r.pass, Some(false));
        assert!(r.failures[0].contains("point-count"), "{:?}", r.failures);
        let ev = r
            .probes
            .iter()
            .find(|p| p.name == "v(a)")
            .expect("evidence probe");
        assert_eq!(ev.aligned_points, Some(5));
        assert!(
            (ev.aligned_abs_err.unwrap() - 1e-12).abs() < 1e-15,
            "{:?}",
            ev.aligned_abs_err
        );
        assert_eq!(ev.point_errors.len(), 5);
        assert!(ev.point_errors.iter().all(|e| e.abs_err < 1e-11));
        // evidence is not a verdict: no pass/fail of its own, no headline error, no vacuous mark
        assert_eq!(
            (ev.pass, ev.rs_err, ev.abs_err, ev.vacuous),
            (None, None, None, false)
        );
        assert_eq!(r.err_rs, None);
        // a grid that differs at point 3 (same length): the three aligned points are the evidence
        let mut ax: Vec<f64> = (0..=5).map(|k| k as f64).collect();
        ax[3] = 3.3;
        let r = run(&m, deck, &mk(ax), &rf);
        assert_eq!(r.pass, Some(false));
        let ev = r.probes.iter().find(|p| p.name == "v(a)").unwrap();
        assert_eq!(ev.aligned_points, Some(3));
        // long sweeps are thinned to at most 64 points
        let long = |n: usize, off: f64| {
            let ax: Vec<f64> = (0..n).map(|k| k as f64).collect();
            let y: Vec<f64> = ax
                .iter()
                .map(|x| x * 0.5 + if *x == 700.0 { off } else { 0.0 })
                .collect();
            vec![plot(
                "DC transfer characteristic",
                &[("v(v-sweep)", ax), ("v(a)", y)],
            )]
        };
        let r = run(
            &m,
            "t\n.dc v1 0 1000 1\n",
            &long(999, 1e-3),
            &long(1000, 0.0),
        );
        let ev = r.probes.iter().find(|p| p.name == "v(a)").unwrap();
        assert_eq!(ev.aligned_points, Some(999));
        assert!(ev.point_errors.len() <= 64, "{}", ev.point_errors.len());
        assert_eq!(ev.aligned_worst_at, Some(700.0));
        assert!(
            ev.point_errors.iter().any(|e| e.at == 700.0),
            "the worst point is kept"
        );
        assert!(
            ev.point_errors.iter().any(|e| e.at == 998.0),
            "the last aligned point is kept"
        );
    }

    /// CAL (c): the error at RustSpice's own rows is reported separately and never decides.
    #[test]
    fn own_grid_error_is_evidence_and_never_changes_the_verdict() {
        let m = meta_from("* bench.tol.norm: 5e-3");
        let deck = "t\n.tran 100n 20u\n";
        // a fast sine, dense exact reference; RustSpice is EXACT at its own rows, but those rows are shifted by a
        // retry (every later row is off the k * tstep grid by 37 ns) and coarse: the nominal-grid comparison
        // linearly interpolates RustSpice's own waveform
        let f = |t: f64| (2.0 * std::f64::consts::PI * 1e5 * t).sin();
        let rf = tran_at(&steps(5e-9, 4000), f);
        let rows: Vec<f64> = (0..=200)
            .map(|k| {
                if k < 50 {
                    k as f64 * 100e-9
                } else {
                    k as f64 * 100e-9 + 37e-9
                }
            })
            .map(|t| t.min(20e-6))
            .collect();
        let rs = tran_at(&rows, f);
        let r = run(&m, deck, &rs, &rf);
        let p = &r.probes[0];
        let own = p.own_grid_err.expect("own-grid evidence");
        let nominal = p.norm_err.unwrap();
        assert!(
            own < 1e-3 * nominal.max(1e-12) + 1e-5,
            "own {own:e} nominal {nominal:e}"
        );
        assert!(
            nominal > 5.0 * own,
            "setup: the nominal-grid error {nominal:e} must exceed the own-grid one {own:e}"
        );
        // the verdict is decided by the nominal grid only
        assert_eq!(r.pass, Some(p.norm_err.unwrap() <= 5e-3));
        // evidence is present for an ordinary pass too
        let good = tran_at(&steps(100e-9, 200), f);
        let r = run(&m, deck, &good, &rf);
        assert!(r.probes[0].own_grid_err.is_some());
        assert_eq!(
            r.probes[0].oracle_own_grid_err, None,
            "no oracle on this case"
        );
    }

    /// ER-metrics-08: the oracle cut-off is reported as `budget`, once per case, with the other probes skipped.
    #[test]
    fn oracle_budget_is_reported_once_per_case() {
        let m = meta_from("* bench.oracle: dc_nonlinear\n* bench.probes: v(d150), v(d1)");
        let mut deck = String::from("diode string\nV1 n0 0 1000\nR1 n0 d1 1000\n");
        for k in 1..300 {
            deck.push_str(&format!("D{k} d{k} d{} dm\n", k + 1));
        }
        deck.push_str("D300 d300 0 dm\n.model dm D(is=1e-14 n=1)\n.op\n.end\n");
        let rf = op(&[("v(d150)", 0.5), ("v(d1)", 0.7)]);
        let t0 = std::time::Instant::now();
        let r = compare_budgeted(
            &CompareInput {
                meta: &m,
                deck: &deck,
                rs: &rf,
                reference: &rf,
                asis: None,
            },
            None,
            Some(Duration::from_millis(300)),
        );
        let o = r.oracle.expect("oracle summary");
        assert_eq!(o.status, "budget", "{:?}", o.reason);
        assert!(o.reason.unwrap().contains("work budget exceeded"));
        assert_eq!(o.rs_max_err, None, "no value, not 0");
        assert!(t0.elapsed() < Duration::from_secs(10), "{:?}", t0.elapsed());
        // the comparison itself is unaffected by the oracle
        assert_eq!(r.pass, Some(true));
    }

    /// ER-metrics-07 end to end: the forward-error bound of the oracle is carried to the case record.
    #[test]
    fn oracle_forward_error_bound_is_reported() {
        let m = meta_from("* bench.oracle: dc_linear\n* bench.probes: v(2)");
        let deck = "t\nV1 1 0 1\nR1 1 2 1k\nR2 2 0 1k\n.op\n.end\n";
        let rf = op(&[("v(2)", 0.5)]);
        let r = run(&m, deck, &rf, &rf);
        let o = r.oracle.unwrap();
        assert_eq!(o.status, "ok");
        let b = o.fwd_bound.expect("a dense solve was made");
        assert!(b > 0.0 && b < 1e-12, "{b:e}");
        assert!(o.rs_max_err.unwrap() < 1e-12);
    }

    /// ER-driver-06: `expect: either` under a slug that does not describe a crash can neither confirm nor close a gap.
    #[test]
    fn either_with_a_non_crash_gap_is_never_reported_as_closed() {
        let ei = meta_from_unchecked(
            "* bench.expect: either\n* bench.known_gap: clamp-rail\n* bench.gap.note: test gap",
        );
        let v = classify(&ei, &ok(), &RsSide::Ok, Some(true));
        assert_eq!(v.status, Status::Info);
        assert!(
            v.notes.iter().any(|n| n.contains("contradictory")),
            "{:?}",
            v.notes
        );
        let v = classify(&ei, &ok(), &RsSide::LoudError("x".into()), None);
        assert_eq!(v.status, Status::Info);
        // a crash slug keeps the closed/confirmed semantics
        let rb = meta_from("* bench.expect: either\n* bench.known_gap: resource-budget\n* bench.gap.note: test gap");
        assert_eq!(
            classify(&rb, &ok(), &RsSide::LoudError("x".into()), None).status,
            Status::GapClosed
        );
        assert_eq!(
            classify(&rb, &ok(), &RsSide::Panic("p".into()), None).status,
            Status::KnownGapConfirmed
        );
    }
}
