//! Independent f64 oracles (owner: H2). No RustSpice solver code, no ngspice code: everything here is written
//! from textbook numerics (dense LU, damped Newton, Radau IIA collocation) and validated against closed forms.
//!
//! `evaluate(deck, spec, probe, axis)` is the single entry point used by the engine for `* bench.oracle: <spec>`
//! (`evaluate_with_budget` takes an explicit [`OracleBudget`]).
//! `axis` is the sweep variable (times for `.tran`, sweep values for `.dc`, `[0.0]` for `.op`). It returns one value
//! per axis point for `probe` (lower case, `v(node)` or `i(vsource)` / `i(inductor)`), or `Err(reason)` when the spec or
//! probe is unknown or the deck lies outside the supported subset. The engine reports `oracle_status: "unsupported"` for
//! an `Err` (never a pass, never zero). The oracle never panics (internal panics are caught and reported as `Err`).
//! An evaluation also has a wall-clock budget (default `min(10 s, case timeout / 2)`, see [`default_wall_budget`]),
//! checked at every factorisation column, Newton iteration, sweep point and integration step: an oracle runs in-process,
//! outside the child watchdog of the suite, so it must never be able to consume the case budget. A cut-off is an `Err`
//! for which [`is_budget_error`] holds (the engine reports `oracle_status: "budget"`), and the other probes of the same
//! deck fail at once from a one-entry negative cache instead of repeating the budget.
//!
//! # Specs
//!
//! * `dc_linear` - dense f64 LU with partial pivoting and one step of iterative refinement (residual evaluated in
//!   compensated "dot2" arithmetic). Elements: `R V I G` (VCCS), flat netlist, `.param` numbers/expressions, all SPICE
//!   suffixes (`t g meg k mil m u n p f`, trailing unit letters ignored). Analyses: `.op`, and `.dc <vsrc|isrc> ...`
//!   (axis = sweep values). A backward-error check rejects numerically singular systems, and a componentwise
//!   forward-error estimate (LAPACK `xGERFS` style: `||| A^-1 | (|r| + u (|A||x| + |b|)) ||_inf / ||x||_inf`, the 1-norm
//!   of `|A^-1|` estimated by Hager/Higham from the LU) rejects systems that the rounding of their own stamps can move by
//!   more than [`FWD_BOUND_LIMIT`] = 1e-8 of the solution norm: a tiny residual says little about an ill-conditioned
//!   solution, while a merely badly scaled matrix (a 1 nohm divider, `cond_1 = 1e18`) is solved to full precision and
//!   accepted. The bound is available from [`take_forward_error_bound`].
//!   Dense limit: [`DC_DENSE_LIMIT`] = 2000 MNA unknowns (32 MiB matrix + copy, ~3 s); larger decks return `Err`
//!   (the 1-D ladder family has its own exact O(n) recursion for those sizes).
//! * `dc_nonlinear` / `dc_nonlinear(gmin=1e-12)` - adds diodes `D` (Shockley `IS`, `N`, with `gmin` in parallel across
//!   every junction; no `RS/BV/CJO/TT`, any other model parameter or instance parameter is an `Err`). Damped Newton in
//!   the weighted-residual merit with a line search, source-stepping fallback, sweep points continued from the previous
//!   point. Thermal voltage is `k*T/q` at 300.15 K with `k = 1.38064852e-23`, `q = 1.6021766208e-19` (the values that
//!   reproduce, to 3e-15 relative, the junction voltage ngspice-47 prints for a 1 mA / 1e-14 A diode; CODATA-2019 would
//!   differ by 3.4e-7 relative). Dense limit [`NL_DENSE_LIMIT`] = 800 unknowns.
//!
//! Any spec accepts a `budget=<seconds>` option, e.g. `dc_nonlinear(gmin=1e-12,budget=30)` or `tran_lti(budget=20)`: the
//! wall-clock budget of that deck's oracle (capped at 80 % of the case timeout), for the few oracles that need more
//! than the default.
//!
//! * `ladder(n=<stages>,rs=<ohm>,rsh=<ohm>[,v=<volt>])` - the uniform open-ended 1-D ladder `V1 n0 0 v; Rs_k n_{k-1}
//!   n_k rs; Rp_k n_k 0 rsh` (the `ladder_dc` family) in closed form, for decks far beyond the dense limit (up to
//!   [`LADDER_MAX_N`] stages). With `cosh(theta) = 1 + rs/(2 rsh)` the exact node voltages are
//!   `v_k = v cosh(theta (n - k + 1/2)) / cosh(theta (n + 1/2))` (the open end gives the half-stage offset) and the source
//!   current `i(v1) = -2 v sinh(theta n) sinh(theta/2) / (rs cosh(theta (n + 1/2)))`, evaluated without cancellation
//!   (`theta = 2 asinh(sqrt(rs/rsh)/2)`). The deck text is not read: the oracle describes the INTENDED circuit, so a deck
//!   that does not implement it disagrees with it. Probes `v(n<k>)` (0 <= k <= n) and `i(v1)`; `.op` only.
//! * `tran_lti` - dense MNA `A x + E x' = b(t)` for linear `R C L V I G` (no mutual inductors), sources `DC`, `PULSE`
//!   (explicit 7 arguments, non-zero rise/fall), `SIN(vo va freq [td [theta [0]]])`, `PWL`. The integrator is the
//!   3-stage Radau IIA collocation method (order 5, L-stable, stiffly accurate) in the stage-value formulation, run on a
//!   grid whose segment boundaries are the exact source breakpoints, with dense output (the cubic collocation polynomial; quadratic through
//!   the stage values in the first step after a kink; exact state at grid points) at the requested times. The step is halved until two consecutive refinements agree to `1e-8` of the probe swing
//!   (plus `1e-13` of its largest magnitude); a run that cannot reach this within the work budget is an `Err`. The
//!   initial state is the DC operating point at `t = 0` (capacitors open, inductors short; `.ic v(n)=x` forces
//!   node voltages during that operating point; `uic` and element-level `ic=` are an `Err`). <= [`TRAN_UNKNOWN_LIMIT`]
//!   = 400 unknowns.
//!
//! Output sign convention: `i(vname)` is the current flowing INTO the `+` terminal through the source to `-` (SPICE
//! convention), `i(lname)` flows from the first to the second node of the inductor.
//!
//! The pure helpers (`parse_number`, `lu_*`, [`RADAU_C`]...) are `pub` so the generators' tests can reuse them.

// Dense numerical kernels index several arrays with one loop variable and test `!(x > y)` on purpose so that NaN
// compares as failure.
#![allow(clippy::needless_range_loop, clippy::neg_cmp_op_on_partial_ord)]

use std::cell::Cell;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Maximum MNA unknowns for the dense `dc_linear` oracle.
pub const DC_DENSE_LIMIT: usize = 2000;
/// Maximum MNA unknowns for the dense `dc_nonlinear` oracle (raised from 500: the 24 x 24 diode-resistor mesh of the corpus has 722).
pub const NL_DENSE_LIMIT: usize = 800;
/// Maximum MNA unknowns for `tran_lti`.
pub const TRAN_UNKNOWN_LIMIT: usize = 400;

/// Largest stage count of the closed-form `ladder` oracle.
pub const LADDER_MAX_N: usize = 100_000_000;

// ------------------------------------------------------------------------------------------------ budgets

/// Wall-clock cap of one oracle evaluation when the driver does not pass a smaller one (see [`default_wall_budget`]).
pub const DEFAULT_WALL_BUDGET: Duration = Duration::from_secs(10);

/// Message prefix of every budget failure (the wall-clock cap and the operation count of `tran_lti`). The step-halving
/// fallback of `tran_lti` and [`is_budget_error`] key on it.
const BUDGET_PREFIX: &str = "work budget exceeded";

/// Operations charged to every transient step on top of the dense-algebra count: a step allocates its stage vectors,
/// builds the source vector and walks the element list, which for a 2-3 unknown circuit costs far more than the
/// `28 n^2` multiply-adds of the algebra (the count alone let a 3-unknown pulse train run 45 s before the cap hit).
const STEP_OVERHEAD_OPS: f64 = 2000.0;

static CASE_TIMEOUT_MS: AtomicU64 = AtomicU64::new(0);

/// Tell the oracles the per-case timeout of the run (the driver calls this once with `--case-timeout`). The default
/// wall-clock budget of an oracle evaluation is then `min(10 s, case_timeout / 2)`: the oracle runs in-process, outside
/// the child watchdog, so without a cap it could consume the whole case budget (or hang the suite).
pub fn set_case_timeout(case_timeout: Duration) {
    let ms = u64::try_from(case_timeout.as_millis()).unwrap_or(u64::MAX);
    CASE_TIMEOUT_MS.store(ms, Ordering::Relaxed);
}

/// `min(10 s, case_timeout / 2)`.
pub fn wall_budget_for_case_timeout(case_timeout: Duration) -> Duration {
    DEFAULT_WALL_BUDGET.min(case_timeout / 2)
}

/// `--case-timeout <seconds>` of the running process, for a driver that has not called [`set_case_timeout`].
fn case_timeout_from_args() -> Option<Duration> {
    static ARGS: OnceLock<Option<Duration>> = OnceLock::new();
    *ARGS.get_or_init(|| {
        let args: Vec<String> = std::env::args().collect();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            let v = if a == "--case-timeout" {
                it.next().cloned()
            } else {
                a.strip_prefix("--case-timeout=").map(str::to_string)
            };
            if let Some(secs) = v.and_then(|v| v.parse::<u64>().ok()).filter(|s| *s > 0) {
                return Some(Duration::from_secs(secs));
            }
        }
        None
    })
}

/// The wall-clock budget [`evaluate`] uses: `min(10 s, case_timeout / 2)` from [`set_case_timeout`] or the process's
/// `--case-timeout` argument, [`DEFAULT_WALL_BUDGET`] when the case timeout is unknown.
pub fn default_wall_budget() -> Duration {
    let ms = CASE_TIMEOUT_MS.load(Ordering::Relaxed);
    if ms > 0 {
        return wall_budget_for_case_timeout(Duration::from_millis(ms));
    }
    case_timeout_from_args().map_or(DEFAULT_WALL_BUDGET, wall_budget_for_case_timeout)
}

/// Resource budget of one [`evaluate_with_budget`] call. An evaluation that exceeds it returns `Err` (a message that
/// [`is_budget_error`] recognises), never a number and never a hang.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OracleBudget {
    /// Wall-clock time of the whole evaluation (parse, factorisations, Newton iterations, integration steps).
    pub wall: Duration,
}

impl OracleBudget {
    pub fn new(wall: Duration) -> Self {
        OracleBudget { wall }
    }
}

impl Default for OracleBudget {
    fn default() -> Self {
        OracleBudget {
            wall: default_wall_budget(),
        }
    }
}

/// Longest wall-clock budget a spec may ask for with its `budget=<seconds>` option when the case timeout is unknown.
pub const MAX_SPEC_BUDGET: Duration = Duration::from_secs(600);

/// Split an optional `budget=<seconds>` option out of a normalised (lower-case, no blanks) spec:
/// `dc_nonlinear(gmin=1e-12,budget=30)` -> (`dc_nonlinear(gmin=1e-12)`, 30 s), `tran_lti(budget=20)` -> (`tran_lti`, 20 s).
/// A deck whose oracle legitimately needs longer than the default budget (a 700-unknown diode mesh solves in ~20 s)
/// says so in its `* bench.oracle:` line; the request is capped at 80 % of the case timeout when that is known (the
/// oracle runs inside the case) and at [`MAX_SPEC_BUDGET`] otherwise.
fn split_budget_option(spec: &str) -> Result<(String, Option<Duration>), String> {
    let Some(open) = spec.find('(') else {
        return Ok((spec.to_string(), None));
    };
    let Some(inner) = spec[open + 1..].strip_suffix(')') else {
        return Ok((spec.to_string(), None));
    };
    let mut budget = None;
    let mut rest: Vec<&str> = Vec::new();
    for part in inner.split(',').filter(|p| !p.is_empty()) {
        match part.split_once('=') {
            Some(("budget", v)) => {
                let secs = parse_number(v)
                    .filter(|x| x.is_finite() && *x > 0.0)
                    .ok_or_else(|| format!("bad budget value '{v}' (seconds expected)"))?;
                let cap = match (
                    CASE_TIMEOUT_MS.load(Ordering::Relaxed),
                    case_timeout_from_args(),
                ) {
                    (ms, _) if ms > 0 => Duration::from_millis(ms).mul_f64(0.8),
                    (_, Some(t)) => t.mul_f64(0.8),
                    _ => MAX_SPEC_BUDGET,
                };
                budget =
                    Some(Duration::from_secs_f64(secs.min(MAX_SPEC_BUDGET.as_secs_f64())).min(cap));
            }
            _ => rest.push(part),
        }
    }
    if budget.is_none() {
        return Ok((spec.to_string(), None));
    }
    let head = &spec[..open];
    let core = if rest.is_empty() {
        head.to_string()
    } else {
        format!("{head}({})", rest.join(","))
    };
    Ok((core, budget))
}

/// `true` for the `Err` of an evaluation that ran out of its budget (wall clock or operation count): the oracle did not
/// fail on the deck, it was cut off.
pub fn is_budget_error(msg: &str) -> bool {
    msg.starts_with(BUDGET_PREFIX)
}

thread_local! {
    /// Deadline (and the budget it came from) of the evaluation running on this thread.
    static DEADLINE: Cell<Option<(Instant, Duration)>> = const { Cell::new(None) };
    /// Largest forward-error bound of the dense linear solves of the last evaluation.
    static FWD_BOUND: Cell<Option<f64>> = const { Cell::new(None) };
    /// Key and message of the last evaluation that was cut off by its budget: the other probes of the same deck
    /// fail at once instead of repeating the whole budget (a cut-off run leaves nothing in the solution caches).
    static NEG_CACHE: std::cell::RefCell<Option<(u64, String)>> = const { std::cell::RefCell::new(None) };
}

struct DeadlineGuard(Option<(Instant, Duration)>);

impl DeadlineGuard {
    fn set(wall: Duration) -> Self {
        let deadline = Instant::now().checked_add(wall).map(|t| (t, wall));
        DeadlineGuard(DEADLINE.with(|d| d.replace(deadline)))
    }
}

impl Drop for DeadlineGuard {
    fn drop(&mut self) {
        DEADLINE.with(|d| d.set(self.0));
    }
}

/// `Err` once the running evaluation is past its wall-clock deadline; free of cost outside an evaluation.
#[inline]
fn check_deadline() -> Result<(), String> {
    DEADLINE.with(|d| match d.get() {
        Some((t, wall)) if Instant::now() >= t => Err(format!(
            "{BUDGET_PREFIX}: oracle wall clock {:.1} s",
            wall.as_secs_f64()
        )),
        _ => Ok(()),
    })
}

fn record_fwd_bound(b: f64) {
    FWD_BOUND.with(|c| c.set(Some(c.get().map_or(b, |x| x.max(b)))));
}

/// Largest componentwise forward-error bound (relative to the solution norm) of the dense linear solves of the
/// last [`evaluate`] on this thread: every sweep point of `dc_linear` (and of a `dc_nonlinear` deck without diodes), the
/// initial operating point of `tran_lti`. `None` when no linear system was solved (Newton iterations and the closed-form
/// ladder are not covered). Read-and-clear. A difference between RustSpice and the oracle below this bound is the
/// oracle's own rounding, not a RustSpice error.
pub fn take_forward_error_bound() -> Option<f64> {
    FWD_BOUND.with(|c| c.take())
}

const K_BOLTZ: f64 = 1.380_648_52e-23;
const Q_ELEC: f64 = 1.602_176_620_8e-19;
const T_KELVIN: f64 = 300.15;
/// Thermal voltage `kT/q` at 300.15 K.
pub const VT: f64 = K_BOLTZ * T_KELVIN / Q_ELEC;
const DEFAULT_GMIN: f64 = 1e-12;
/// Exponent above which the diode law continues linearly (overflow guard; unreachable in real solutions).
const ARG_MAX: f64 = 80.0;

/// Exact 1-norm condition number `|A|_1 |A^-1|_1` of the MNA matrix of a flat linear deck (the `dc_linear` subset, at
/// most 64 unknowns: the inverse is formed column by column from the LU factors). The fuzzers use it to keep every
/// generated comparison well conditioned: the relative error of any solver is at most about `cond * 1.1e-16`.
pub fn dc_condition(deck: &str) -> Result<f64, String> {
    let r = catch_unwind(AssertUnwindSafe(|| -> Result<f64, String> {
        let net = parse_deck(deck)?;
        let m = build_mna(&net, false, false, 64)?;
        let n = m.n;
        let norm1 = |a: &[f64]| {
            (0..n)
                .map(|j| (0..n).map(|i| a[i * n + j].abs()).sum::<f64>())
                .fold(0.0f64, f64::max)
        };
        let mut lu = m.a.clone();
        let piv = lu_factor(n, &mut lu)?;
        let mut inv = vec![0.0; n * n];
        for j in 0..n {
            let mut e = vec![0.0; n];
            e[j] = 1.0;
            lu_solve(n, &lu, &piv, &mut e);
            for (i, v) in e.iter().enumerate() {
                inv[i * n + j] = *v;
            }
        }
        let c = norm1(&m.a) * norm1(&inv);
        if c.is_finite() {
            Ok(c)
        } else {
            Err("non-finite condition number".to_string())
        }
    }));
    r.unwrap_or_else(|_| Err("oracle internal error (panic caught)".to_string()))
}

/// Evaluate `probe` of `deck` under oracle `spec` on `axis`. See the module documentation. The evaluation is cut off
/// after [`default_wall_budget`] (an `Err` for which [`is_budget_error`] holds); use [`evaluate_with_budget`] to choose.
pub fn evaluate(deck: &str, spec: &str, probe: &str, axis: &[f64]) -> Result<Vec<f64>, String> {
    evaluate_with_budget(deck, spec, probe, axis, &OracleBudget::default())
}

/// [`evaluate`] with an explicit [`OracleBudget`].
pub fn evaluate_with_budget(
    deck: &str,
    spec: &str,
    probe: &str,
    axis: &[f64],
    budget: &OracleBudget,
) -> Result<Vec<f64>, String> {
    FWD_BOUND.with(|c| c.set(None));
    let _guard = DeadlineGuard::set(budget.wall);
    let wall_ms = u64::try_from(budget.wall.as_millis()).unwrap_or(u64::MAX);
    let r = catch_unwind(AssertUnwindSafe(|| {
        evaluate_inner(deck, spec, probe, axis, wall_ms)
    }));
    match r {
        Ok(v) => v,
        Err(_) => Err("oracle internal error (panic caught)".to_string()),
    }
}

fn evaluate_inner(
    deck: &str,
    spec: &str,
    probe: &str,
    axis: &[f64],
    wall_ms: u64,
) -> Result<Vec<f64>, String> {
    if axis.is_empty() {
        return Err("empty axis".to_string());
    }
    if axis.iter().any(|x| !x.is_finite()) {
        return Err("non-finite axis value".to_string());
    }
    let spec_n: String = spec
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    // a `budget=<s>` option of the spec replaces the budget of the caller for this evaluation
    let (spec_n, spec_budget) = split_budget_option(&spec_n)?;
    let _spec_guard = spec_budget.map(DeadlineGuard::set);
    let wall_ms = spec_budget.map_or(wall_ms, |b| {
        u64::try_from(b.as_millis()).unwrap_or(u64::MAX)
    });
    let kind = parse_spec(&spec_n)?;
    let probe = probe.trim().to_ascii_lowercase();
    if let Spec::Ladder { n, rs, rsh, vin } = kind {
        return ladder_eval(n, rs, rsh, vin, &probe, axis);
    }
    let net = parse_deck(deck)?;
    let mut key = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut key, deck.as_bytes());
    fnv(&mut key, spec_n.as_bytes());
    for t in axis {
        fnv(&mut key, &t.to_bits().to_le_bytes());
    }
    fnv(&mut key, &wall_ms.to_le_bytes());
    if let Some(msg) = NEG_CACHE.with(|c| {
        c.borrow()
            .as_ref()
            .and_then(|(k, m)| (*k == key).then(|| m.clone()))
    }) {
        return Err(msg);
    }
    let r = match kind {
        Spec::DcLinear => dc_eval(&net, &probe, axis, None, key),
        Spec::DcNonlinear(g) => dc_eval(&net, &probe, axis, Some(g), key),
        Spec::TranLti => tran_eval(&net, &probe, axis, deck),
        Spec::Ladder { .. } => Err("internal: ladder spec reached the deck path".to_string()),
    };
    if let Err(e) = &r {
        if is_budget_error(e) {
            NEG_CACHE.with(|c| *c.borrow_mut() = Some((key, e.clone())));
        }
    }
    r
}

#[derive(Clone, Copy)]
enum Spec {
    Ladder {
        n: usize,
        rs: f64,
        rsh: f64,
        vin: f64,
    },
    DcLinear,
    DcNonlinear(f64),
    TranLti,
}

fn parse_spec(s: &str) -> Result<Spec, String> {
    match s {
        "dc_linear" => Ok(Spec::DcLinear),
        "dc_nonlinear" => Ok(Spec::DcNonlinear(DEFAULT_GMIN)),
        "tran_lti" => Ok(Spec::TranLti),
        _ => {
            if let Some(rest) = s
                .strip_prefix("dc_nonlinear(")
                .and_then(|r| r.strip_suffix(')'))
            {
                let mut gmin = DEFAULT_GMIN;
                for part in rest.split(',').filter(|p| !p.is_empty()) {
                    match part.split_once('=') {
                        Some(("gmin", v)) => {
                            gmin =
                                parse_number(v).ok_or_else(|| format!("bad gmin value '{v}'"))?;
                            if !(gmin >= 0.0 && gmin.is_finite()) {
                                return Err("gmin must be finite and >= 0".to_string());
                            }
                        }
                        _ => return Err(format!("unknown dc_nonlinear option '{part}'")),
                    }
                }
                Ok(Spec::DcNonlinear(gmin))
            } else if let Some(rest) = s.strip_prefix("ladder(").and_then(|r| r.strip_suffix(')')) {
                parse_ladder_spec(rest)
            } else {
                Err(format!("unknown oracle spec '{s}'"))
            }
        }
    }
}

fn parse_ladder_spec(rest: &str) -> Result<Spec, String> {
    let (mut n, mut rs, mut rsh, mut vin) = (None, None, None, 1.0);
    for part in rest.split(',').filter(|p| !p.is_empty()) {
        let (k, v) = part
            .split_once('=')
            .ok_or_else(|| format!("bad ladder option '{part}'"))?;
        let x = parse_number(v).ok_or_else(|| format!("bad ladder value '{v}'"))?;
        match k {
            "n" => n = Some(x),
            "rs" => rs = Some(x),
            "rsh" => rsh = Some(x),
            "v" => vin = x,
            _ => return Err(format!("unknown ladder option '{k}'")),
        }
    }
    let (n, rs, rsh) = match (n, rs, rsh) {
        (Some(n), Some(rs), Some(rsh)) => (n, rs, rsh),
        _ => return Err("ladder needs n, rs and rsh".to_string()),
    };
    if !(n >= 1.0 && n <= LADDER_MAX_N as f64 && n == n.trunc()) {
        return Err(format!("ladder n must be an integer in 1..={LADDER_MAX_N}"));
    }
    if !(rs > 0.0 && rsh > 0.0 && rs.is_finite() && rsh.is_finite() && vin.is_finite()) {
        return Err("ladder rs and rsh must be positive and finite".to_string());
    }
    Ok(Spec::Ladder {
        n: n as usize,
        rs,
        rsh,
        vin,
    })
}

/// Closed form of the open-ended uniform ladder, see the module documentation.
fn ladder_eval(
    n: usize,
    rs: f64,
    rsh: f64,
    vin: f64,
    probe: &str,
    axis: &[f64],
) -> Result<Vec<f64>, String> {
    let theta = 2.0 * (0.5 * (rs / rsh).sqrt()).asinh();
    let nf = n as f64;
    let den = (theta * (nf + 0.5)).cosh();
    let v = if let Some(k) = probe
        .strip_prefix("v(n")
        .and_then(|r| r.strip_suffix(')'))
        .and_then(|r| r.parse::<usize>().ok())
        .filter(|&k| k <= n)
    {
        vin * (theta * (nf - k as f64 + 0.5)).cosh() / den
    } else if probe == "i(v1)" {
        -2.0 * vin * (theta * nf).sinh() * (0.5 * theta).sinh() / (rs * den)
    } else {
        return Err(format!("ladder oracle has no probe '{probe}'"));
    };
    if !v.is_finite() {
        return Err("ladder closed form overflowed".to_string());
    }
    Ok(vec![v; axis.len()])
}

// ------------------------------------------------------------------------------------------------ numbers

/// Parse a SPICE number with an optional scale suffix: `t g meg k mil m u n p f a` (case-insensitive; `a` = 1e-18 as in
/// ngspice, where `5A` is five atto, not five amperes); any further letters (units such as `v`, `ohm`, `hz`) are
/// ignored. Returns `None` if no number is found.
pub fn parse_number(tok: &str) -> Option<f64> {
    let b = tok.trim().to_ascii_lowercase();
    let (v, pos) = scan_number(b.as_bytes(), 0)?;
    if pos != b.len() {
        // allow trailing letters only (units); scan_number consumed suffix letters already
        return None;
    }
    Some(v)
}

/// Scan a number (with suffix and trailing unit letters) at `pos`; returns value and new position.
fn scan_number(b: &[u8], mut pos: usize) -> Option<(f64, usize)> {
    let start = pos;
    if pos < b.len() && (b[pos] == b'+' || b[pos] == b'-') {
        pos += 1;
    }
    let ds = pos;
    while pos < b.len() && b[pos].is_ascii_digit() {
        pos += 1;
    }
    let mut ndig = pos - ds;
    if pos < b.len() && b[pos] == b'.' {
        pos += 1;
        let fs = pos;
        while pos < b.len() && b[pos].is_ascii_digit() {
            pos += 1;
        }
        ndig += pos - fs;
    }
    if ndig == 0 {
        return None;
    }
    if pos < b.len() && b[pos] == b'e' {
        let mut q = pos + 1;
        if q < b.len() && (b[q] == b'+' || b[q] == b'-') {
            q += 1;
        }
        if q < b.len() && b[q].is_ascii_digit() {
            while q < b.len() && b[q].is_ascii_digit() {
                q += 1;
            }
            pos = q;
        }
    }
    let mant: f64 = std::str::from_utf8(&b[start..pos]).ok()?.parse().ok()?;
    let ls = pos;
    while pos < b.len() && b[pos].is_ascii_alphabetic() {
        pos += 1;
    }
    let suf = std::str::from_utf8(&b[ls..pos]).ok()?;
    let scale = if suf.starts_with("meg") {
        1e6
    } else if suf.starts_with("mil") {
        25.4e-6
    } else {
        match suf.as_bytes().first() {
            Some(b't') => 1e12,
            Some(b'g') => 1e9,
            Some(b'k') => 1e3,
            Some(b'm') => 1e-3,
            Some(b'u') => 1e-6,
            Some(b'n') => 1e-9,
            Some(b'p') => 1e-12,
            Some(b'f') => 1e-15,
            // atto: ngspice-47 reads `1a`, `5A` and `3amp` as 1e-18, 5e-18 and 3e-18 (black-box check), so a unit
            // letter `a` is a scale factor, not a unit
            Some(b'a') => 1e-18,
            _ => 1.0,
        }
    };
    let v = mant * scale;
    if v.is_finite() {
        Some((v, pos))
    } else {
        None
    }
}

// Tiny recursive-descent expression evaluator for `{...}` / `'...'` parameter expressions.
struct ExprP<'a> {
    b: &'a [u8],
    pos: usize,
    params: &'a BTreeMap<String, f64>,
}

impl ExprP<'_> {
    fn ws(&mut self) {
        while self.pos < self.b.len() && self.b[self.pos] == b' ' {
            self.pos += 1;
        }
    }
    fn expr(&mut self) -> Result<f64, String> {
        let mut v = self.term()?;
        loop {
            self.ws();
            match self.b.get(self.pos) {
                Some(b'+') => {
                    self.pos += 1;
                    v += self.term()?;
                }
                Some(b'-') => {
                    self.pos += 1;
                    v -= self.term()?;
                }
                _ => return Ok(v),
            }
        }
    }
    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.power()?;
        loop {
            self.ws();
            match self.b.get(self.pos) {
                Some(b'*') if self.b.get(self.pos + 1) != Some(&b'*') => {
                    self.pos += 1;
                    v *= self.power()?;
                }
                Some(b'/') => {
                    self.pos += 1;
                    let d = self.power()?;
                    if d == 0.0 {
                        return Err("division by zero in expression".to_string());
                    }
                    v /= d;
                }
                _ => return Ok(v),
            }
        }
    }
    fn power(&mut self) -> Result<f64, String> {
        let base = self.unary()?;
        self.ws();
        let is_pow = match (self.b.get(self.pos), self.b.get(self.pos + 1)) {
            (Some(b'*'), Some(b'*')) => {
                self.pos += 2;
                true
            }
            (Some(b'^'), _) => {
                self.pos += 1;
                true
            }
            _ => false,
        };
        if is_pow {
            let e = self.power()?;
            return Ok(base.powf(e));
        }
        Ok(base)
    }
    fn unary(&mut self) -> Result<f64, String> {
        self.ws();
        match self.b.get(self.pos) {
            Some(b'-') => {
                self.pos += 1;
                Ok(-self.unary()?)
            }
            Some(b'+') => {
                self.pos += 1;
                self.unary()
            }
            _ => self.primary(),
        }
    }
    fn primary(&mut self) -> Result<f64, String> {
        self.ws();
        match self.b.get(self.pos) {
            None => Err("unexpected end of expression".to_string()),
            Some(b'(') => {
                self.pos += 1;
                let v = self.expr()?;
                self.ws();
                if self.b.get(self.pos) != Some(&b')') {
                    return Err("missing ')' in expression".to_string());
                }
                self.pos += 1;
                Ok(v)
            }
            Some(c) if c.is_ascii_digit() || *c == b'.' => {
                let (v, p) = scan_number(self.b, self.pos).ok_or("bad number in expression")?;
                self.pos = p;
                Ok(v)
            }
            Some(c) if c.is_ascii_alphabetic() || *c == b'_' => {
                let s = self.pos;
                while self.pos < self.b.len()
                    && (self.b[self.pos].is_ascii_alphanumeric() || self.b[self.pos] == b'_')
                {
                    self.pos += 1;
                }
                let name = std::str::from_utf8(&self.b[s..self.pos]).unwrap_or("");
                self.ws();
                if self.b.get(self.pos) == Some(&b'(') {
                    self.pos += 1;
                    let a = self.expr()?;
                    self.ws();
                    if self.b.get(self.pos) != Some(&b')') {
                        return Err("missing ')' after function argument".to_string());
                    }
                    self.pos += 1;
                    return match name {
                        "sqrt" => Ok(a.sqrt()),
                        "abs" => Ok(a.abs()),
                        "exp" => Ok(a.exp()),
                        "ln" | "log" => Ok(a.ln()),
                        "log10" => Ok(a.log10()),
                        "sin" => Ok(a.sin()),
                        "cos" => Ok(a.cos()),
                        _ => Err(format!("unsupported function '{name}' in expression")),
                    };
                }
                match name {
                    "pi" => Ok(std::f64::consts::PI),
                    _ => self
                        .params
                        .get(name)
                        .copied()
                        .ok_or_else(|| format!("unknown parameter '{name}'")),
                }
            }
            Some(c) => Err(format!(
                "unexpected character '{}' in expression",
                *c as char
            )),
        }
    }
}

fn eval_value(tok: &str, params: &BTreeMap<String, f64>) -> Result<f64, String> {
    let t = tok.trim();
    let inner = if (t.starts_with('{') && t.ends_with('}') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        &t[1..t.len() - 1]
    } else {
        t
    };
    if let Some(v) = parse_number(inner) {
        return Ok(v);
    }
    let mut p = ExprP {
        b: inner.as_bytes(),
        pos: 0,
        params,
    };
    let v = p.expr().map_err(|e| format!("{e} in '{tok}'"))?;
    p.ws();
    if p.pos != p.b.len() {
        return Err(format!("trailing characters in value '{tok}'"));
    }
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!("non-finite value '{tok}'"))
    }
}

// ------------------------------------------------------------------------------------------------ deck parsing

/// Split a logical line into tokens: whitespace/`,`/`(`/`)` separate tokens at brace depth 0; `{..}` and `'..'` stay
/// whole; `=` glues its neighbours (`a = 2` -> `a=2`).
fn tokenize(line: &str) -> Vec<String> {
    let mut raw: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut inq = false;
    for ch in line.chars() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
        } else if ch == '\'' {
            inq = !inq;
        }
        let sep = depth <= 0 && !inq && (ch.is_whitespace() || ch == ',' || ch == '(' || ch == ')');
        if sep {
            if !cur.is_empty() {
                raw.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        raw.push(cur);
    }
    let mut out: Vec<String> = Vec::new();
    let mut glue_next = false;
    for t in raw {
        if glue_next || t.starts_with('=') {
            if let Some(last) = out.last_mut() {
                last.push_str(&t);
                glue_next = t.ends_with('=');
                continue;
            }
        }
        glue_next = t.ends_with('=');
        out.push(t);
    }
    out
}

/// Logical (comment-stripped, continuation-joined, lower-cased) lines after the title line.
fn logical_lines(deck: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, raw) in deck.lines().enumerate() {
        if i == 0 {
            continue; // title line
        }
        let mut line = raw.trim().to_string();
        if let Some(p) = line.find(';') {
            line.truncate(p);
        }
        if let Some(p) = line.find(" $") {
            line.truncate(p);
        }
        let line = line.trim();
        if line.is_empty() || line.starts_with('*') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('+') {
            if let Some(last) = out.last_mut() {
                last.push(' ');
                last.push_str(rest.trim());
                continue;
            }
        }
        out.push(line.to_ascii_lowercase());
    }
    out
}

#[derive(Clone, Debug)]
enum Wave {
    None,
    Pulse {
        v1: f64,
        v2: f64,
        td: f64,
        tr: f64,
        tf: f64,
        pw: f64,
        per: f64,
    },
    Sin {
        vo: f64,
        va: f64,
        freq: f64,
        td: f64,
        theta: f64,
    },
    Pwl(Vec<(f64, f64)>),
}

#[derive(Clone, Debug)]
struct Source {
    dc: Option<f64>,
    wave: Wave,
}

impl Source {
    fn wave_at(&self, t: f64) -> f64 {
        match &self.wave {
            Wave::None => self.dc.unwrap_or(0.0),
            Wave::Pulse {
                v1,
                v2,
                td,
                tr,
                tf,
                pw,
                per,
            } => {
                if t < *td {
                    return *v1;
                }
                let mut p = t - td;
                if p >= *per {
                    p -= (p / per).floor() * per;
                    if p < 0.0 {
                        p = 0.0;
                    }
                }
                if p < *tr {
                    v1 + (v2 - v1) * p / tr
                } else if p < tr + pw {
                    *v2
                } else if p < tr + pw + tf {
                    v2 + (v1 - v2) * (p - tr - pw) / tf
                } else {
                    *v1
                }
            }
            Wave::Sin {
                vo,
                va,
                freq,
                td,
                theta,
            } => {
                if t < *td {
                    *vo
                } else {
                    let u = t - td;
                    vo + va * (2.0 * std::f64::consts::PI * freq * u).sin() * (-u * theta).exp()
                }
            }
            Wave::Pwl(pts) => {
                if t <= pts[0].0 {
                    return pts[0].1;
                }
                for w in pts.windows(2) {
                    if t <= w[1].0 {
                        let (t0, v0) = w[0];
                        let (t1, v1) = w[1];
                        return v0 + (v1 - v0) * (t - t0) / (t1 - t0);
                    }
                }
                pts[pts.len() - 1].1
            }
        }
    }
    /// DC (operating point) value.
    fn dc_value(&self) -> f64 {
        match self.dc {
            Some(v) => v,
            None => self.wave_at(0.0),
        }
    }
    /// Append breakpoints (times where the waveform has a kink) in `(0, tmax)`.
    fn breakpoints(&self, tmax: f64, out: &mut Vec<f64>) {
        match &self.wave {
            Wave::None => {}
            Wave::Pulse {
                td,
                tr,
                tf,
                pw,
                per,
                ..
            } => {
                let mut k = 0u64;
                loop {
                    let base = td + (k as f64) * per;
                    if base >= tmax || k > 2_000_000 {
                        break;
                    }
                    for off in [0.0, *tr, tr + pw, tr + pw + tf] {
                        let t = base + off;
                        if t > 0.0 && t < tmax {
                            out.push(t);
                        }
                    }
                    k += 1;
                }
            }
            Wave::Sin { td, .. } => {
                if *td > 0.0 && *td < tmax {
                    out.push(*td);
                }
            }
            Wave::Pwl(p) => {
                for (t, _) in p {
                    if *t > 0.0 && *t < tmax {
                        out.push(*t);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
enum Elem {
    R {
        a: usize,
        b: usize,
        r: f64,
    },
    C {
        a: usize,
        b: usize,
        c: f64,
    },
    L {
        name: String,
        a: usize,
        b: usize,
        l: f64,
    },
    V {
        name: String,
        a: usize,
        b: usize,
        src: Source,
    },
    I {
        name: String,
        a: usize,
        b: usize,
        src: Source,
    },
    G {
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        gm: f64,
    },
    D {
        a: usize,
        k: usize,
        is: f64,
        n: f64,
    },
}

#[derive(Clone, Debug)]
enum Analysis {
    Op,
    Dc { src: String },
    Tran { tstop: f64 },
}

struct Net {
    node_names: Vec<String>, // index 0 = ground placeholder
    elems: Vec<Elem>,
    analyses: Vec<Analysis>,
    ic: Vec<(usize, f64)>,
}

fn node_id(map: &mut BTreeMap<String, usize>, names: &mut Vec<String>, n: &str) -> usize {
    if n == "0" || n == "gnd" {
        return 0;
    }
    if let Some(&i) = map.get(n) {
        return i;
    }
    let i = names.len();
    names.push(n.to_string());
    map.insert(n.to_string(), i);
    i
}

fn parse_source(
    toks: &[String],
    params: &BTreeMap<String, f64>,
    tstep_hint: f64,
) -> Result<Source, String> {
    let _ = tstep_hint;
    let mut dc: Option<f64> = None;
    let mut wave = Wave::None;
    let mut i = 0;
    let is_kw = |t: &str| {
        matches!(
            t,
            "dc" | "ac"
                | "pulse"
                | "sin"
                | "pwl"
                | "exp"
                | "sffm"
                | "am"
                | "trnoise"
                | "trrandom"
                | "external"
        )
    };
    while i < toks.len() {
        let t = toks[i].as_str();
        match t {
            "dc" => {
                let v = toks.get(i + 1).ok_or("DC without value")?;
                dc = Some(eval_value(v, params)?);
                i += 2;
            }
            "ac" => {
                // AC magnitude/phase do not influence DC or transient results.
                i += 1;
                while i < toks.len() && !is_kw(&toks[i]) {
                    i += 1;
                }
            }
            "pulse" | "sin" | "pwl" => {
                let mut j = i + 1;
                let mut args: Vec<String> = Vec::new();
                while j < toks.len() && !is_kw(&toks[j]) {
                    args.push(toks[j].clone());
                    j += 1;
                }
                if args.iter().any(|a| a.contains('=')) {
                    return Err(format!("{t}: named/option arguments are not supported"));
                }
                let vals: Result<Vec<f64>, String> =
                    args.iter().map(|a| eval_value(a, params)).collect();
                let vals = vals?;
                wave = match t {
                    "pulse" => {
                        if vals.len() != 7 {
                            return Err(
                                "PULSE needs exactly 7 explicit arguments (v1 v2 td tr tf pw per)"
                                    .to_string(),
                            );
                        }
                        let (v1, v2, td, tr, tf, pw, per) = (
                            vals[0], vals[1], vals[2], vals[3], vals[4], vals[5], vals[6],
                        );
                        if tr <= 0.0 || tf <= 0.0 {
                            return Err("PULSE with zero rise/fall time is simulator-defined; not supported".to_string());
                        }
                        if td < 0.0 || pw < 0.0 || per <= 0.0 || per < tr + pw + tf {
                            return Err("PULSE: invalid delay/width/period".to_string());
                        }
                        Wave::Pulse {
                            v1,
                            v2,
                            td,
                            tr,
                            tf,
                            pw,
                            per,
                        }
                    }
                    "sin" => {
                        if !(3..=6).contains(&vals.len()) {
                            return Err("SIN needs 3..6 arguments".to_string());
                        }
                        let td = vals.get(3).copied().unwrap_or(0.0);
                        let theta = vals.get(4).copied().unwrap_or(0.0);
                        let phase = vals.get(5).copied().unwrap_or(0.0);
                        if phase != 0.0 {
                            return Err("SIN with non-zero phase not supported".to_string());
                        }
                        if vals[2] <= 0.0 || td < 0.0 {
                            return Err("SIN: frequency must be > 0, delay >= 0".to_string());
                        }
                        Wave::Sin {
                            vo: vals[0],
                            va: vals[1],
                            freq: vals[2],
                            td,
                            theta,
                        }
                    }
                    _ => {
                        if vals.len() < 4 || vals.len() % 2 != 0 {
                            return Err("PWL needs an even number (>= 4) of values".to_string());
                        }
                        let pts: Vec<(f64, f64)> = vals.chunks(2).map(|c| (c[0], c[1])).collect();
                        if pts.windows(2).any(|w| w[1].0 <= w[0].0) {
                            return Err("PWL times must be strictly increasing".to_string());
                        }
                        Wave::Pwl(pts)
                    }
                };
                i = j;
            }
            _ if is_kw(t) => return Err(format!("unsupported source waveform '{t}'")),
            _ => {
                // plain value => DC
                if dc.is_some() {
                    return Err(format!("unexpected token '{t}' in source"));
                }
                dc = Some(eval_value(t, params)?);
                i += 1;
            }
        }
    }
    let s = Source { dc, wave };
    if let (Some(d), true) = (s.dc, !matches!(s.wave, Wave::None)) {
        let w0 = s.wave_at(0.0);
        if (d - w0).abs() > 1e-12 * (1.0 + w0.abs()) {
            return Err("source has both DC value and a waveform with different t=0 value (simulator-defined)".to_string());
        }
    }
    Ok(s)
}

fn parse_deck(deck: &str) -> Result<Net, String> {
    let lines = logical_lines(deck);
    let mut params: BTreeMap<String, f64> = BTreeMap::new();
    // pass 1: .param (repeat for forward references)
    let mut pending: Vec<Vec<String>> = lines
        .iter()
        .filter(|l| l.starts_with(".param"))
        .map(|l| tokenize(l))
        .collect();
    for _ in 0..8 {
        if pending.is_empty() {
            break;
        }
        let mut again = Vec::new();
        for toks in pending.drain(..) {
            let mut rest = Vec::new();
            for t in &toks[1..] {
                match t.split_once('=') {
                    Some((k, v)) => match eval_value(v, &params) {
                        Ok(x) => {
                            params.insert(k.to_string(), x);
                        }
                        Err(_) => rest.push(t.clone()),
                    },
                    None => return Err(format!(".param token '{t}' without '='")),
                }
            }
            if !rest.is_empty() {
                let mut v = vec![".param".to_string()];
                v.extend(rest);
                again.push(v);
            }
        }
        pending = again;
    }
    if !pending.is_empty() {
        return Err("unresolvable .param expression".to_string());
    }
    // pass 2: models
    let mut models: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for l in lines.iter().filter(|l| l.starts_with(".model")) {
        let t = tokenize(l);
        if t.len() < 3 {
            return Err("malformed .model".to_string());
        }
        if t[2] != "d" {
            return Err(format!(".model type '{}' not supported (only D)", t[2]));
        }
        let (mut is, mut n) = (1e-14, 1.0);
        for p in &t[3..] {
            match p.split_once('=') {
                Some(("is", v)) => is = eval_value(v, &params)?,
                Some(("n", v)) => n = eval_value(v, &params)?,
                Some((k, _)) => return Err(format!("diode model parameter '{k}' not supported")),
                None => return Err(format!("diode model token '{p}' not supported")),
            }
        }
        if !(is > 0.0 && n > 0.0) {
            return Err("diode IS and N must be > 0".to_string());
        }
        models.insert(t[1].clone(), (is, n));
    }
    // pass 3: elements and analyses
    let mut node_map: BTreeMap<String, usize> = BTreeMap::new();
    let mut node_names = vec!["0".to_string()];
    let mut elems = Vec::new();
    let mut analyses = Vec::new();
    let mut ic = Vec::new();
    for l in &lines {
        if l.starts_with('.') {
            let t = tokenize(l);
            match t[0].as_str() {
                ".end" => break,
                ".param" | ".model" | ".nodeset" | ".save" | ".print" | ".plot" | ".probe"
                | ".width" | ".meas" | ".measure" => {}
                ".op" => analyses.push(Analysis::Op),
                ".options" | ".option" => {
                    for p in &t[1..] {
                        let key = p.split('=').next().unwrap_or("");
                        if matches!(
                            key,
                            "temp"
                                | "tnom"
                                | "gmin"
                                | "rshunt"
                                | "cshunt"
                                | "gshunt"
                                | "rabstol"
                                | "scale"
                        ) {
                            return Err(format!(
                                ".options {key} changes the physics; not supported"
                            ));
                        }
                    }
                }
                ".ic" => {
                    // tokens: v ( n ) = x  -> tokenizer splits "v(n1)=1" into ["v","n1","=1"] merged -> "v","n1=1"
                    let mut i = 1;
                    while i < t.len() {
                        if t[i] == "v" && i + 1 < t.len() {
                            let (n, v) = t[i + 1].split_once('=').ok_or("malformed .ic entry")?;
                            let id = node_id(&mut node_map, &mut node_names, n);
                            if id == 0 {
                                return Err(".ic on ground node".to_string());
                            }
                            ic.push((id, eval_value(v, &params)?));
                            i += 2;
                        } else {
                            return Err(format!("unsupported .ic token '{}'", t[i]));
                        }
                    }
                }
                ".dc" => {
                    if t.len() != 5 {
                        return Err(".dc: only a single source sweep '.dc src start stop step' is supported".to_string());
                    }
                    let nm = &t[1];
                    if !(nm.starts_with('v') || nm.starts_with('i')) {
                        return Err(".dc: only V/I sources can be swept".to_string());
                    }
                    analyses.push(Analysis::Dc { src: nm.clone() });
                }
                ".tran" => {
                    if t.len() < 3 {
                        return Err("malformed .tran".to_string());
                    }
                    if t.iter().skip(3).any(|x| x == "uic") {
                        return Err(".tran ... uic is not supported by tran_lti".to_string());
                    }
                    let tstop = eval_value(&t[2], &params)?;
                    analyses.push(Analysis::Tran { tstop });
                }
                other => return Err(format!("unsupported dot card '{other}'")),
            }
            continue;
        }
        let t = tokenize(l);
        let name = t[0].clone();
        let kind = name.as_bytes()[0];
        let nid = |s: &str, m: &mut BTreeMap<String, usize>, n: &mut Vec<String>| node_id(m, n, s);
        match kind {
            b'r' | b'c' | b'l' => {
                if t.len() != 4 {
                    return Err(format!(
                        "{name}: only 'name n1 n2 value' is supported (found {} tokens)",
                        t.len()
                    ));
                }
                let a = nid(&t[1], &mut node_map, &mut node_names);
                let b = nid(&t[2], &mut node_map, &mut node_names);
                let v = eval_value(&t[3], &params).map_err(|e| format!("{name}: {e}"))?;
                if a == b {
                    return Err(format!("{name}: both terminals on the same node"));
                }
                if !(v > 0.0) {
                    return Err(format!("{name}: value must be > 0"));
                }
                elems.push(match kind {
                    b'r' => Elem::R { a, b, r: v },
                    b'c' => Elem::C { a, b, c: v },
                    _ => Elem::L {
                        name: name.clone(),
                        a,
                        b,
                        l: v,
                    },
                });
            }
            b'v' | b'i' => {
                if t.len() < 4 {
                    return Err(format!("{name}: too few tokens"));
                }
                let a = nid(&t[1], &mut node_map, &mut node_names);
                let b = nid(&t[2], &mut node_map, &mut node_names);
                if a == b {
                    return Err(format!("{name}: both terminals on the same node"));
                }
                let src =
                    parse_source(&t[3..], &params, 0.0).map_err(|e| format!("{name}: {e}"))?;
                elems.push(if kind == b'v' {
                    Elem::V {
                        name: name.clone(),
                        a,
                        b,
                        src,
                    }
                } else {
                    Elem::I {
                        name: name.clone(),
                        a,
                        b,
                        src,
                    }
                });
            }
            b'g' => {
                if t.len() != 6 {
                    return Err(format!("{name}: only 'g n+ n- nc+ nc- gm' is supported"));
                }
                let a = nid(&t[1], &mut node_map, &mut node_names);
                let b = nid(&t[2], &mut node_map, &mut node_names);
                let c = nid(&t[3], &mut node_map, &mut node_names);
                let d = nid(&t[4], &mut node_map, &mut node_names);
                let gm = eval_value(&t[5], &params).map_err(|e| format!("{name}: {e}"))?;
                elems.push(Elem::G { a, b, c, d, gm });
            }
            b'd' => {
                if t.len() != 4 {
                    return Err(format!(
                        "{name}: only 'd anode cathode model' is supported (no area/params)"
                    ));
                }
                let a = nid(&t[1], &mut node_map, &mut node_names);
                let k = nid(&t[2], &mut node_map, &mut node_names);
                let (is, n) = *models
                    .get(&t[3])
                    .ok_or_else(|| format!("{name}: unknown model '{}'", t[3]))?;
                elems.push(Elem::D { a, k, is, n });
            }
            _ => return Err(format!("unsupported element '{name}'")),
        }
    }
    Ok(Net {
        node_names,
        elems,
        analyses,
        ic,
    })
}

// ------------------------------------------------------------------------------------------------ dense linear algebra

/// Error-free product: returns (p, e) with `a*b = p + e` exactly (via FMA).
#[inline]
fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    (p, a.mul_add(b, -p))
}

/// Error-free sum: returns (s, e) with `a+b = s + e` exactly.
#[inline]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

/// Compensated dot product (twice working precision): `sum a[i]*b[i]`.
pub fn dot2(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    let mut c = 0.0;
    for (&x, &y) in a.iter().zip(b) {
        let (p, pe) = two_prod(x, y);
        let (ns, se) = two_sum(s, p);
        s = ns;
        c += se + pe;
    }
    s + c
}

/// In-place LU with partial pivoting of the row-major `n x n` matrix; returns the pivot permutation.
pub fn lu_factor(n: usize, a: &mut [f64]) -> Result<Vec<usize>, String> {
    let mut piv = vec![0usize; n];
    let amax = a.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if !amax.is_finite() || amax == 0.0 {
        return Err("matrix is zero or non-finite".to_string());
    }
    for k in 0..n {
        check_deadline()?;
        let mut p = k;
        let mut best = a[k * n + k].abs();
        for i in k + 1..n {
            let v = a[i * n + k].abs();
            if v > best {
                best = v;
                p = i;
            }
        }
        if !(best > amax * 1e-18) {
            return Err(format!(
                "singular or numerically singular matrix (pivot {k})"
            ));
        }
        piv[k] = p;
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
        }
        let d = a[k * n + k];
        for i in k + 1..n {
            let l = a[i * n + k] / d;
            a[i * n + k] = l;
            if l != 0.0 {
                let (top, bot) = a.split_at_mut(i * n);
                let rk = &top[k * n + k + 1..k * n + n];
                let ri = &mut bot[k + 1..n];
                for (x, y) in ri.iter_mut().zip(rk) {
                    *x -= l * *y;
                }
            }
        }
    }
    Ok(piv)
}

/// Solve with a factorisation from [`lu_factor`]; `b` is overwritten with the solution.
pub fn lu_solve(n: usize, lu: &[f64], piv: &[usize], b: &mut [f64]) {
    // rows were swapped as a whole in `lu_factor` (LAPACK convention): apply every interchange first, then L.
    for k in 0..n {
        let p = piv[k];
        if p != k {
            b.swap(k, p);
        }
    }
    for k in 0..n {
        let bk = b[k];
        if bk != 0.0 {
            for i in k + 1..n {
                b[i] -= lu[i * n + k] * bk;
            }
        }
    }
    for k in (0..n).rev() {
        let mut s = b[k];
        for j in k + 1..n {
            s -= lu[k * n + j] * b[j];
        }
        b[k] = s / lu[k * n + k];
    }
}

/// Solve `A^T x = b` with a factorisation from [`lu_factor`] (`P A = L U`, so `A^T = U^T L^T P`); `b` is overwritten.
pub fn lu_solve_t(n: usize, lu: &[f64], piv: &[usize], b: &mut [f64]) {
    // U^T z = b (forward substitution, U^T is lower triangular)
    for k in 0..n {
        let mut s = b[k];
        for j in 0..k {
            s -= lu[j * n + k] * b[j];
        }
        b[k] = s / lu[k * n + k];
    }
    // L^T w = z (backward substitution, unit diagonal)
    for k in (0..n).rev() {
        let mut s = b[k];
        for j in k + 1..n {
            s -= lu[j * n + k] * b[j];
        }
        b[k] = s;
    }
    // x = P^T w: undo the interchanges in reverse order
    for k in (0..n).rev() {
        let p = piv[k];
        if p != k {
            b.swap(k, p);
        }
    }
}

/// Hager / Higham estimate of the 1-norm of a matrix that is only available through products: `mul(v)` overwrites `v`
/// with `C v`, `mul_t(v)` with `C^T v`. A lower bound that equals the norm in nearly all cases and is never off by more
/// than a small factor in practice; a handful of products. `INFINITY` when a product overflows.
fn norm1_estimate(n: usize, mul: &dyn Fn(&mut [f64]), mul_t: &dyn Fn(&mut [f64])) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let mut x = vec![1.0 / n as f64; n];
    let mut est = 0.0f64;
    let mut jlast = usize::MAX;
    for iter in 0..5 {
        let mut y = x.clone();
        mul(&mut y);
        let ynorm: f64 = y.iter().map(|v| v.abs()).sum();
        if !ynorm.is_finite() {
            return f64::INFINITY;
        }
        if iter > 0 && ynorm <= est {
            break;
        }
        est = est.max(ynorm);
        let mut z: Vec<f64> = y
            .iter()
            .map(|v| if *v >= 0.0 { 1.0 } else { -1.0 })
            .collect();
        mul_t(&mut z);
        let (mut j, mut zmax) = (0usize, 0.0f64);
        for (i, v) in z.iter().enumerate() {
            if v.abs() > zmax {
                zmax = v.abs();
                j = i;
            }
        }
        let ztx: f64 = z.iter().zip(&x).map(|(a, b)| a * b).sum();
        if !zmax.is_finite() {
            return f64::INFINITY;
        }
        if zmax <= ztx || j == jlast {
            break;
        }
        x = vec![0.0; n];
        x[j] = 1.0;
        jlast = j;
    }
    // Higham's alternating-sign test vector guards against a start vector orthogonal to the dominant direction
    let mut v: Vec<f64> = (0..n)
        .map(|i| {
            let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
            sign * (1.0 + i as f64 / (n.max(2) - 1) as f64)
        })
        .collect();
    mul(&mut v);
    let alt = 2.0 * v.iter().map(|a| a.abs()).sum::<f64>() / (3.0 * n as f64);
    if alt.is_finite() {
        est.max(alt)
    } else {
        f64::INFINITY
    }
}

/// Estimate of `||A^-1||_1` from the LU factors.
fn inv_norm1_estimate(n: usize, lu: &[f64], piv: &[usize]) -> f64 {
    norm1_estimate(n, &|v| lu_solve(n, lu, piv, v), &|v| {
        lu_solve_t(n, lu, piv, v)
    })
}

/// Componentwise forward-error bound of a refined solution (the LAPACK `xGERFS` estimate): the data of the system are
/// the rounded stamps `a_ij`, `b_i` (relative error `u` each), so the solution is uncertain by
/// `||| A^-1 | (|r| + u (|A||x| + |b|)) ||_inf`, relative to `||x||_inf`. Unlike the normwise `cond_1(A) u` it does not
/// punish a matrix that is merely badly SCALED (a 1 nohm divider has `cond_1 = 1e18` and is solved to full precision):
/// only an error that the conductances' own rounding can really amplify counts. `|A^-1| w` is estimated through the
/// 1-norm of `diag(w) A^-T`. `mag` is the matrix of the magnitudes of the stamps (`|a_ij|`, or the SUM of the absolute
/// values of the individual stamps when entries cancel, e.g. a negative `G` against a resistor: the cancellation error
/// of the stamping is a data error of that size, not of the size of the difference).
fn forward_error_bound(
    n: usize,
    mag: &[f64],
    b: &[f64],
    x: &[f64],
    r: &[f64],
    lu: &[f64],
    piv: &[usize],
) -> f64 {
    // the bound is homogeneous in (x, r, b): work with them divided by ||x||_inf so that a solution near the overflow
    // limit (a 1e308 V source) does not overflow `|A||x|` while the system itself is perfectly conditioned
    let xnorm = x.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if !xnorm.is_finite() {
        return f64::INFINITY;
    }
    let unit = if xnorm > 0.0 { xnorm } else { 1.0 };
    let w: Vec<f64> = (0..n)
        .map(|i| {
            let row = &mag[i * n..(i + 1) * n];
            let s: f64 = row
                .iter()
                .zip(x)
                .map(|(a, x)| a * (x.abs() / unit))
                .sum::<f64>()
                + b[i].abs() / unit;
            r[i].abs() / unit + UNIT_ROUNDOFF * s
        })
        .collect();
    let est = norm1_estimate(
        n,
        // C v = diag(w) A^-T v
        &|v| {
            lu_solve_t(n, lu, piv, v);
            for (vi, wi) in v.iter_mut().zip(&w) {
                *vi *= wi;
            }
        },
        // C^T v = A^-1 (w .* v)
        &|v| {
            for (vi, wi) in v.iter_mut().zip(&w) {
                *vi *= wi;
            }
            lu_solve(n, lu, piv, v);
        },
    );
    if est == 0.0 {
        0.0
    } else if xnorm > 0.0 {
        est
    } else {
        f64::INFINITY
    }
}

/// Unit roundoff of f64 (`2^-53`).
pub const UNIT_ROUNDOFF: f64 = 1.110_223_024_625_156_5e-16;

/// Largest forward-error bound (relative to `||x||_inf`, see [`forward_error_bound`]) [`solve_dense_refined`] accepts.
/// The oracle arbitrates differences with RustSpice at the suite's default tolerances (1e-9 absolute, 1e-6 relative); a
/// solution that the rounding of its own data (the stamped conductances) can already move by more than 1e-8 of its norm
/// cannot decide a difference at that level.
pub const FWD_BOUND_LIMIT: f64 = 1e-8;

thread_local! {
    /// Limit in force on this thread (tests lower or lift it to look at the bound of a rejected system).
    static FWD_LIMIT: Cell<f64> = const { Cell::new(FWD_BOUND_LIMIT) };
}

/// Result of [`solve_dense_refined_report`].
#[derive(Clone, Debug)]
pub struct RefinedSolve {
    pub x: Vec<f64>,
    /// Componentwise backward error `max_i |r_i| / (|A||x| + |b|)_i` of the refined solution.
    pub backward_error: f64,
    /// Estimate of the 1-norm condition number `|A|_1 |A^-1|_1` (informational: a badly SCALED matrix has a huge one
    /// without being a problem, see [`forward_error_bound`]).
    pub cond_est: f64,
    /// Componentwise forward-error bound relative to `||x||_inf`: how far the rounding of the data of the system
    /// (relative `1.1e-16` per stamped value) can move the solution.
    pub forward_bound: f64,
}

/// Solve `A x = b` (row-major dense) by LU + one step of iterative refinement with a compensated residual, and report
/// the condition estimate and the forward-error bound. Errors only when the system is singular, non-finite or the final
/// backward error exceeds `1e-10`; whether the forward-error bound is acceptable is the caller's decision
/// ([`solve_dense_refined`] applies [`FWD_BOUND_LIMIT`]).
pub fn solve_dense_refined_report(n: usize, a: &[f64], b: &[f64]) -> Result<RefinedSolve, String> {
    solve_refined(n, a, None, b)
}

/// [`solve_dense_refined_report`] with the matrix of stamp magnitudes (`None`: `|a|`).
fn solve_refined(
    n: usize,
    a: &[f64],
    mag: Option<&[f64]>,
    b: &[f64],
) -> Result<RefinedSolve, String> {
    let mut lu = a.to_vec();
    let piv = lu_factor(n, &mut lu)?;
    let mut x = b.to_vec();
    lu_solve(n, &lu, &piv, &mut x);
    let mut r = vec![0.0; n];
    let resid = |x: &[f64], r: &mut [f64]| {
        for i in 0..n {
            // r_i = b_i - A_i x with compensated accumulation
            let row = &a[i * n..(i + 1) * n];
            let mut s = b[i];
            let mut c = 0.0;
            for (&aij, &xj) in row.iter().zip(x) {
                let (p, pe) = two_prod(-aij, xj);
                let (ns, se) = two_sum(s, p);
                s = ns;
                c += se + pe;
            }
            r[i] = s + c;
        }
    };
    resid(&x, &mut r);
    let mut d = r.clone();
    lu_solve(n, &lu, &piv, &mut d);
    for i in 0..n {
        x[i] += d[i];
    }
    resid(&x, &mut r);
    // backward error check
    let mut worst = 0.0f64;
    for i in 0..n {
        let row = &a[i * n..(i + 1) * n];
        let scale: f64 = row.iter().zip(&x).map(|(a, x)| (a * x).abs()).sum::<f64>() + b[i].abs();
        if scale > 0.0 {
            worst = worst.max(r[i].abs() / scale);
        } else if r[i] != 0.0 {
            worst = f64::INFINITY;
        }
    }
    if !(worst <= 1e-10) || x.iter().any(|v| !v.is_finite()) {
        return Err(format!(
            "ill-conditioned system (backward error {worst:.2e})"
        ));
    }
    check_deadline()?;
    let norm1_a = (0..n)
        .map(|j| (0..n).map(|i| a[i * n + j].abs()).sum::<f64>())
        .fold(0.0f64, f64::max);
    let cond_est = norm1_a * inv_norm1_estimate(n, &lu, &piv);
    let abs_a: Vec<f64>;
    let mag = match mag {
        Some(m) => m,
        None => {
            abs_a = a.iter().map(|v| v.abs()).collect();
            &abs_a
        }
    };
    let forward_bound = forward_error_bound(n, mag, b, &x, &r, &lu, &piv);
    Ok(RefinedSolve {
        x,
        backward_error: worst,
        cond_est,
        forward_bound,
    })
}

/// [`solve_dense_refined_report`] that also rejects a system whose componentwise forward-error bound exceeds
/// [`FWD_BOUND_LIMIT`]: such a system passes the backward-error test (a tiny residual says little about an
/// ill-conditioned solution) yet its solution may be wrong in the leading digits. The bound of every solve, rejected
/// or not, is recorded for [`take_forward_error_bound`].
pub fn solve_dense_refined(n: usize, a: &[f64], b: &[f64]) -> Result<Vec<f64>, String> {
    solve_checked(n, a, None, b)
}

/// [`solve_dense_refined`] with the matrix of stamp magnitudes.
fn solve_checked(n: usize, a: &[f64], mag: Option<&[f64]>, b: &[f64]) -> Result<Vec<f64>, String> {
    let r = solve_refined(n, a, mag, b)?;
    record_fwd_bound(r.forward_bound);
    let limit = FWD_LIMIT.with(|c| c.get());
    if !(r.forward_bound <= limit) {
        return Err(format!(
            "ill-conditioned system (forward-error bound {:.1e} of the solution norm > {:.0e}; condition estimate {:.2e})",
            r.forward_bound, limit, r.cond_est
        ));
    }
    Ok(r.x)
}

// ------------------------------------------------------------------------------------------------ MNA assembly

struct Mna {
    n: usize,
    nn: usize,
    /// linear conductance/topology matrix (row-major), `A x + E x' = b`
    a: Vec<f64>,
    /// magnitudes of the stamps of `a` (sum of the absolute values of every stamp of an entry): the data uncertainty
    /// of the entry, larger than `|a_ij|` where stamps cancel
    am: Vec<f64>,
    /// dynamic matrix (row-major)
    e: Vec<f64>,
    vbranch: BTreeMap<String, usize>,
    lbranch: BTreeMap<String, usize>,
    /// (unknown index of branch row, element index) for V sources; I sources handled directly
    vrows: Vec<(usize, usize)>,
    diodes: Vec<(usize, usize, f64, f64)>, // (a idx or usize::MAX, k idx, is, n)
    has_dynamic: bool,
}

const GND: usize = usize::MAX;

fn nidx(x: usize) -> usize {
    if x == 0 {
        GND
    } else {
        x - 1
    }
}

fn build_mna(
    net: &Net,
    allow_diode: bool,
    allow_dynamic: bool,
    limit: usize,
) -> Result<Mna, String> {
    let nn = net.node_names.len() - 1;
    let mut nb = 0usize;
    for el in &net.elems {
        if matches!(el, Elem::V { .. } | Elem::L { .. }) {
            nb += 1;
        }
    }
    let n = nn + nb;
    if n == 0 {
        return Err("empty circuit".to_string());
    }
    if n > limit {
        return Err(format!(
            "{n} MNA unknowns exceed the dense oracle limit of {limit}"
        ));
    }
    let mut a = vec![0.0; n * n];
    let mut am = vec![0.0; n * n];
    let mut e = vec![0.0; n * n];
    let mut m = Mna {
        n,
        nn,
        a: Vec::new(),
        am: Vec::new(),
        e: Vec::new(),
        vbranch: BTreeMap::new(),
        lbranch: BTreeMap::new(),
        vrows: Vec::new(),
        diodes: Vec::new(),
        has_dynamic: false,
    };
    let add = |mat: &mut Vec<f64>, r: usize, c: usize, v: f64| {
        if r != GND && c != GND {
            mat[r * n + c] += v;
        }
    };
    // stamp into the linear matrix and into its magnitude matrix
    let mut stamp = |a: &mut Vec<f64>, r: usize, c: usize, v: f64| {
        if r != GND && c != GND {
            a[r * n + c] += v;
            am[r * n + c] += v.abs();
        }
    };
    let mut next = nn;
    for (ei, el) in net.elems.iter().enumerate() {
        match el {
            Elem::R { a: p, b: q, r } => {
                let g = 1.0 / r;
                let (p, q) = (nidx(*p), nidx(*q));
                stamp(&mut a, p, p, g);
                stamp(&mut a, q, q, g);
                stamp(&mut a, p, q, -g);
                stamp(&mut a, q, p, -g);
            }
            Elem::C { a: p, b: q, c } => {
                if !allow_dynamic {
                    // open circuit in DC analyses
                    continue;
                }
                m.has_dynamic = true;
                let (p, q) = (nidx(*p), nidx(*q));
                add(&mut e, p, p, *c);
                add(&mut e, q, q, *c);
                add(&mut e, p, q, -*c);
                add(&mut e, q, p, -*c);
            }
            Elem::L {
                name,
                a: p,
                b: q,
                l,
            } => {
                let br = next;
                next += 1;
                m.lbranch.insert(name.clone(), br);
                let (p, q) = (nidx(*p), nidx(*q));
                stamp(&mut a, p, br, 1.0);
                stamp(&mut a, q, br, -1.0);
                stamp(&mut a, br, p, 1.0);
                stamp(&mut a, br, q, -1.0);
                if allow_dynamic {
                    m.has_dynamic = true;
                    e[br * n + br] += -*l;
                }
            }
            Elem::V {
                name, a: p, b: q, ..
            } => {
                let br = next;
                next += 1;
                m.vbranch.insert(name.clone(), br);
                m.vrows.push((br, ei));
                let (p, q) = (nidx(*p), nidx(*q));
                stamp(&mut a, p, br, 1.0);
                stamp(&mut a, q, br, -1.0);
                stamp(&mut a, br, p, 1.0);
                stamp(&mut a, br, q, -1.0);
            }
            Elem::I { .. } => {}
            Elem::G {
                a: p,
                b: q,
                c,
                d,
                gm,
            } => {
                let (p, q, c, d) = (nidx(*p), nidx(*q), nidx(*c), nidx(*d));
                stamp(&mut a, p, c, *gm);
                stamp(&mut a, p, d, -*gm);
                stamp(&mut a, q, c, -*gm);
                stamp(&mut a, q, d, *gm);
            }
            Elem::D { a: p, k, is, n: nf } => {
                if !allow_diode {
                    return Err("diode present: use dc_nonlinear".to_string());
                }
                m.diodes.push((nidx(*p), nidx(*k), *is, *nf));
            }
        }
    }
    m.a = a;
    m.am = am;
    m.e = e;
    Ok(m)
}

/// Right-hand side `b` for source values given by `val(element index, source)`.
fn build_rhs(net: &Net, m: &Mna, val: &dyn Fn(&str, &Source) -> f64) -> Vec<f64> {
    let mut b = vec![0.0; m.n];
    for (br, ei) in &m.vrows {
        if let Elem::V { name, src, .. } = &net.elems[*ei] {
            b[*br] = val(name, src);
        }
    }
    for el in &net.elems {
        if let Elem::I { name, a, b: q, src } = el {
            let v = val(name, src);
            let (p, q) = (nidx(*a), nidx(*q));
            if p != GND {
                b[p] -= v;
            }
            if q != GND {
                b[q] += v;
            }
        }
    }
    b
}

fn probe_index(net: &Net, m: &Mna, probe: &str, allow_l: bool) -> Result<usize, String> {
    if let Some(n) = probe.strip_prefix("v(").and_then(|r| r.strip_suffix(')')) {
        return match net.node_names.iter().position(|x| x == n) {
            Some(i) if i > 0 => Ok(i - 1),
            _ => Err(format!("probe node '{n}' not found (or ground)")),
        };
    }
    if let Some(n) = probe.strip_prefix("i(").and_then(|r| r.strip_suffix(')')) {
        if let Some(&i) = m.vbranch.get(n) {
            return Ok(i);
        }
        if allow_l {
            if let Some(&i) = m.lbranch.get(n) {
                return Ok(i);
            }
        }
        return Err(format!(
            "probe current '{n}' not a voltage source{}",
            if allow_l { " or inductor" } else { "" }
        ));
    }
    Err(format!("unsupported probe '{probe}'"))
}

// ------------------------------------------------------------------------------------------------ DC

fn diode_iv(vd: f64, is: f64, nf: f64, gmin: f64) -> (f64, f64) {
    let vte = nf * VT;
    let arg = vd / vte;
    let (e, de) = if arg > ARG_MAX {
        let e0 = ARG_MAX.exp();
        (e0 * (1.0 + (arg - ARG_MAX)), e0)
    } else {
        let e = arg.exp();
        (e, e)
    };
    (is * (e - 1.0) + gmin * vd, is * de / vte + gmin)
}

/// Full solution vectors of the last dc evaluation (one per axis point), so that the several probes the engine asks
/// for on one deck solve it once. Deterministic: a hit returns exactly what a miss would have computed.
struct DcCache {
    key: u64,
    xs: Vec<Vec<f64>>,
    /// Forward-error bound of the linear solves that produced `xs` (a hit reports it again).
    fwd: Option<f64>,
}

thread_local! {
    static DC_CACHE: std::cell::RefCell<Option<DcCache>> = const { std::cell::RefCell::new(None) };
}

fn dc_eval(
    net: &Net,
    probe: &str,
    axis: &[f64],
    gmin: Option<f64>,
    key: u64,
) -> Result<Vec<f64>, String> {
    let nonlinear = gmin.is_some();
    let limit = if nonlinear {
        NL_DENSE_LIMIT
    } else {
        DC_DENSE_LIMIT
    };
    let m = build_mna(net, nonlinear, false, limit)?;
    let pidx = probe_index(net, &m, probe, false)?;
    let sweep: Option<&String> = {
        let mut found = None;
        let ops = net
            .analyses
            .iter()
            .filter(|a| matches!(a, Analysis::Op))
            .count();
        let dcs: Vec<&String> = net
            .analyses
            .iter()
            .filter_map(|a| {
                if let Analysis::Dc { src } = a {
                    Some(src)
                } else {
                    None
                }
            })
            .collect();
        // `.op` plot: axis == [0.0] (also for a deck that carries both `.op` and `.dc`)
        if axis.len() == 1 && axis[0] == 0.0 && ops > 0 {
            // op
        } else if dcs.len() == 1 {
            found = Some(dcs[0]);
        } else if dcs.len() > 1 {
            return Err("multiple .dc cards".to_string());
        } else if ops == 0 {
            return Err("deck has no .op/.dc analysis".to_string());
        } else {
            return Err("axis has several points but the deck has no .dc".to_string());
        }
        found
    };
    if let Some(s) = sweep {
        let exists = net
            .elems
            .iter()
            .any(|e| matches!(e, Elem::V { name, .. } | Elem::I { name, .. } if name == s));
        if !exists {
            return Err(format!(".dc source '{s}' not found"));
        }
    }
    let hit = DC_CACHE.with(|c| {
        c.borrow().as_ref().and_then(|c| {
            (c.key == key && c.xs.len() == axis.len() && c.xs.iter().all(|x| pidx < x.len()))
                .then(|| (c.xs.iter().map(|x| x[pidx]).collect::<Vec<f64>>(), c.fwd))
        })
    });
    if let Some((v, fwd)) = hit {
        if let Some(b) = fwd {
            record_fwd_bound(b);
        }
        return Ok(v);
    }
    let g = gmin.unwrap_or(0.0);
    let mut out = Vec::with_capacity(axis.len());
    let mut xs: Vec<Vec<f64>> = Vec::new();
    let keep = m.n.saturating_mul(axis.len()) <= 4_000_000;
    let mut prev: Option<Vec<f64>> = None;
    for &av in axis {
        check_deadline()?;
        let b = build_rhs(net, &m, &|name, src| match sweep {
            Some(s) if s == name => av,
            _ => src.dc_value(),
        });
        let x = if m.diodes.is_empty() {
            solve_checked(m.n, &m.a, Some(&m.am), &b)?
        } else {
            newton_dc(&m, &b, g, prev.as_deref())?
        };
        out.push(x[pidx]);
        if keep {
            xs.push(x.clone());
        }
        prev = Some(x);
    }
    if keep {
        let fwd = FWD_BOUND.with(|c| c.get());
        DC_CACHE.with(|c| *c.borrow_mut() = Some(DcCache { key, xs, fwd }));
    }
    Ok(out)
}

struct NewtonCtx<'a> {
    m: &'a Mna,
    gmin: f64,
}

impl NewtonCtx<'_> {
    /// Residual `F = A x + diode currents - b` (compensated for the linear part) and weights.
    fn residual(&self, x: &[f64], b: &[f64], f: &mut [f64], w: &mut [f64]) {
        let n = self.m.n;
        for i in 0..n {
            let row = &self.m.a[i * n..(i + 1) * n];
            let mut s = -b[i];
            let mut c = 0.0;
            let mut wt = b[i].abs();
            for (&aij, &xj) in row.iter().zip(x) {
                let (p, pe) = two_prod(aij, xj);
                let (ns, se) = two_sum(s, p);
                s = ns;
                c += se + pe;
                wt += (aij * xj).abs();
            }
            f[i] = s + c;
            w[i] = wt;
        }
        for &(a, k, is, nf) in &self.m.diodes {
            let va = if a == GND { 0.0 } else { x[a] };
            let vk = if k == GND { 0.0 } else { x[k] };
            let (id, _) = diode_iv(va - vk, is, nf, self.gmin);
            if a != GND {
                f[a] += id;
                w[a] += id.abs();
            }
            if k != GND {
                f[k] -= id;
                w[k] += id.abs();
            }
        }
    }
    fn jacobian(&self, x: &[f64]) -> Vec<f64> {
        let n = self.m.n;
        let mut j = self.m.a.clone();
        for &(a, k, is, nf) in &self.m.diodes {
            let va = if a == GND { 0.0 } else { x[a] };
            let vk = if k == GND { 0.0 } else { x[k] };
            let (_, g) = diode_iv(va - vk, is, nf, self.gmin);
            if a != GND {
                j[a * n + a] += g;
            }
            if k != GND {
                j[k * n + k] += g;
            }
            if a != GND && k != GND {
                j[a * n + k] -= g;
                j[k * n + a] -= g;
            }
        }
        j
    }
    fn merit(f: &[f64], w: &[f64]) -> f64 {
        let mut s = 0.0;
        for (fi, wi) in f.iter().zip(w) {
            let r = fi / (wi + 1e-300).max(1e-30);
            s += r * r;
        }
        s.sqrt()
    }
}

/// Damped Newton from `x0` (or zeros). Returns the converged solution or `Err`.
fn newton_from(
    ctx: &NewtonCtx,
    b: &[f64],
    x0: Vec<f64>,
    max_it: usize,
) -> Result<Vec<f64>, String> {
    let n = ctx.m.n;
    let mut x = x0;
    let mut f = vec![0.0; n];
    let mut w = vec![0.0; n];
    ctx.residual(&x, b, &mut f, &mut w);
    let mut merit = NewtonCtx::merit(&f, &w);
    let mut tiny_steps = 0;
    for _ in 0..max_it {
        check_deadline()?;
        let mut jac = ctx.jacobian(&x);
        let piv = lu_factor(n, &mut jac)?;
        let mut dx: Vec<f64> = f.iter().map(|v| -v).collect();
        lu_solve(n, &jac, &piv, &mut dx);
        if dx.iter().any(|v| !v.is_finite()) {
            return Err("Newton step not finite".to_string());
        }
        let xn = x.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let dn = dx.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let mut lam = 1.0;
        let mut xt = vec![0.0; n];
        let mut ft = vec![0.0; n];
        let mut wt = vec![0.0; n];
        let mut mt;
        loop {
            for i in 0..n {
                xt[i] = x[i] + lam * dx[i];
            }
            ctx.residual(&xt, b, &mut ft, &mut wt);
            mt = NewtonCtx::merit(&ft, &wt);
            if mt <= (1.0 - 1e-4 * lam) * merit || mt < 1e-12 || lam < 1e-8 {
                break;
            }
            lam *= 0.5;
        }
        x.copy_from_slice(&xt);
        f.copy_from_slice(&ft);
        w.copy_from_slice(&wt);
        merit = mt;
        if lam == 1.0 && dn <= 1e-6 * (1.0 + xn) {
            tiny_steps += 1;
            if tiny_steps >= 3 {
                return Ok(x);
            }
        } else {
            tiny_steps = 0;
        }
    }
    Err(format!("Newton did not converge (merit {merit:.2e})"))
}

fn newton_dc(m: &Mna, b: &[f64], gmin: f64, prev: Option<&[f64]>) -> Result<Vec<f64>, String> {
    let ctx = NewtonCtx { m, gmin };
    // a budget failure is final: retrying from another start would only spend more of the same budget
    let stop = |r: Result<Vec<f64>, String>| match r {
        Err(e) if is_budget_error(&e) => Err(Some(e)),
        Err(_) => Err(None),
        Ok(x) => Ok(x),
    };
    if let Some(p) = prev {
        match stop(newton_from(&ctx, b, p.to_vec(), 200)) {
            Ok(x) => return Ok(x),
            Err(Some(e)) => return Err(e),
            Err(None) => {}
        }
    }
    match stop(newton_from(&ctx, b, vec![0.0; m.n], 300)) {
        Ok(x) => return Ok(x),
        Err(Some(e)) => return Err(e),
        Err(None) => {}
    }
    // source stepping continuation
    let mut x = vec![0.0; m.n];
    let mut s = 0.0f64;
    let mut ds = 0.1f64;
    let mut guard = 0;
    while s < 1.0 {
        guard += 1;
        check_deadline()?;
        if guard > 400 || ds < 1e-6 {
            return Err("source stepping failed".to_string());
        }
        let st = (s + ds).min(1.0);
        let bs: Vec<f64> = b.iter().map(|v| v * st).collect();
        match newton_from(&ctx, &bs, x.clone(), 100) {
            Ok(xn) => {
                x = xn;
                s = st;
                ds = (ds * 1.5).min(0.25);
            }
            Err(e) if is_budget_error(&e) => return Err(e),
            Err(_) => ds *= 0.5,
        }
    }
    newton_from(&ctx, b, x, 100)
}

// ------------------------------------------------------------------------------------------------ transient

/// Radau IIA (3 stages) nodes `c`.
pub const RADAU_C: [f64; 3] = [0.155_051_025_721_682_2, 0.644_948_974_278_317_8, 1.0];

fn radau_a() -> [[f64; 3]; 3] {
    let s6 = 6.0f64.sqrt();
    [
        [
            (88.0 - 7.0 * s6) / 360.0,
            (296.0 - 169.0 * s6) / 1800.0,
            (-2.0 + 3.0 * s6) / 225.0,
        ],
        [
            (296.0 + 169.0 * s6) / 1800.0,
            (88.0 + 7.0 * s6) / 360.0,
            (-2.0 - 3.0 * s6) / 225.0,
        ],
        [(16.0 - s6) / 36.0, (16.0 + s6) / 36.0, 1.0 / 9.0],
    ]
}

fn inv3(a: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let (i1, i2) = ((i + 1) % 3, (i + 2) % 3);
            let (j1, j2) = ((j + 1) % 3, (j + 2) % 3);
            // adjugate: cofactor of (j,i)
            r[i][j] = (a[j1][i1] * a[j2][i2] - a[j1][i2] * a[j2][i1]) / det;
        }
    }
    r
}

struct RadauLu {
    lu: Vec<f64>,
    piv: Vec<usize>,
}

struct TranSys<'a> {
    net: &'a Net,
    m: Mna,
    ainv: [[f64; 3]; 3],
    rowsum: [f64; 3],
    cache: Vec<(u64, RadauLu)>,
    ops: f64,
}

impl TranSys<'_> {
    fn b_at(&self, t: f64) -> Vec<f64> {
        build_rhs(self.net, &self.m, &|_, s| s.wave_at(t))
    }

    fn factor(&mut self, h: f64) -> Result<usize, String> {
        let key = h.to_bits();
        if let Some(p) = self.cache.iter().position(|(k, _)| *k == key) {
            return Ok(p);
        }
        let n = self.m.n;
        let n3 = 3 * n;
        let mut mm = vec![0.0; n3 * n3];
        for bi in 0..3 {
            for bj in 0..3 {
                let f = self.ainv[bi][bj] / h;
                for r in 0..n {
                    let dst = &mut mm[(bi * n + r) * n3 + bj * n..(bi * n + r) * n3 + bj * n + n];
                    let er = &self.m.e[r * n..(r + 1) * n];
                    if bi == bj {
                        let ar = &self.m.a[r * n..(r + 1) * n];
                        for c in 0..n {
                            dst[c] = f * er[c] + ar[c];
                        }
                    } else {
                        for c in 0..n {
                            dst[c] = f * er[c];
                        }
                    }
                }
            }
        }
        self.ops += (n3 as f64).powi(3) * 0.7;
        let piv = lu_factor(n3, &mut mm)?;
        let cap = ((200usize << 20) / (72 * self.m.n * self.m.n).max(1)).clamp(8, 96);
        while self.cache.len() >= cap {
            self.cache.remove(0);
        }
        self.cache.push((key, RadauLu { lu: mm, piv }));
        Ok(self.cache.len() - 1)
    }

    /// One Radau step from `(t, x)` with size `h`; returns the three stage vectors (the last one is `x(t+h)`).
    fn step(&mut self, t: f64, h: f64, x: &[f64]) -> Result<Vec<f64>, String> {
        let n = self.m.n;
        let ci = self.factor(h)?;
        // E x_n
        let mut ex = vec![0.0; n];
        for (r, exr) in ex.iter_mut().enumerate() {
            let er = &self.m.e[r * n..(r + 1) * n];
            let mut s = 0.0;
            for (a, b) in er.iter().zip(x) {
                s += a * b;
            }
            *exr = s;
        }
        let mut rhs = vec![0.0; 3 * n];
        for i in 0..3 {
            let bi = self.b_at(t + RADAU_C[i] * h);
            let f = self.rowsum[i] / h;
            for r in 0..n {
                rhs[i * n + r] = bi[r] + f * ex[r];
            }
        }
        let (_, lu) = &self.cache[ci];
        lu_solve(3 * n, &lu.lu, &lu.piv, &mut rhs);
        self.ops += 18.0 * (n * n) as f64 + 10.0 * (n * n) as f64 + STEP_OVERHEAD_OPS;
        Ok(rhs)
    }
}

/// Lagrange weights of the polynomial through the stage values `(c_i, Y_i)` and, when `with_start`, the start value
/// `(0, x_n)` (cubic, O(h^4)); without it the quadratic (O(h^3)). The start value is excluded in the FIRST step of a
/// segment: a source kink sits at its start, and with a step much larger than the fastest time constant the stage
/// values are the damped (quasi-steady) solution while `x_n` still carries the fast transient excited by the kink, so a
/// cubic through both would be wrong for the whole step. In every later step `x_n` is itself a damped stage value.
fn dense_weights(theta: f64, with_start: bool) -> [f64; 4] {
    let nodes = [0.0, RADAU_C[0], RADAU_C[1], RADAU_C[2]];
    let lo = if with_start { 0 } else { 1 };
    let mut w = [0.0; 4];
    for j in lo..4 {
        let mut v = 1.0;
        for m in lo..4 {
            if m != j {
                v *= (theta - nodes[m]) / (nodes[j] - nodes[m]);
            }
        }
        w[j] = v;
    }
    w
}

/// Growth factor of the graded steps after a source kink.
const GRADE_RATIO: f64 = 1.2;

/// Run the integrator at one refinement level; returns `y[w][k]` = value of unknown `want[w]` at `times[k]`
/// (`times` sorted ascending).
fn run_level(
    sys: &mut TranSys,
    x0: &[f64],
    want: &[usize],
    times: &[f64],
    grid: &[f64],
    (h_target, h_first): (f64, f64),
    (refine, max_ops): (f64, f64),
) -> Result<Vec<Vec<f64>>, String> {
    let n = sys.m.n;
    let mut out = vec![vec![0.0; times.len()]; want.len()];
    let mut x = x0.to_vec();
    let mut k = 0usize; // next output index
    while k < times.len() && times[k] <= 0.0 {
        for (w, &u) in want.iter().enumerate() {
            out[w][k] = x[u];
        }
        k += 1;
    }
    for seg in grid.windows(2) {
        let (s0, s1) = (seg[0], seg[1]);
        let len = s1 - s0;
        if len <= 0.0 {
            continue;
        }
        // Step sizes: geometric grading away from the segment start (a source kink excites modes far faster than any
        // sensible uniform step; the grading resolves them at the cost of ~30 shared factorisations), then uniform.
        // A segment shorter than the global step gets its own refinement ladder (at least 4 steps at level 0, doubling
        // with every level, `refine` = 2^level), so the step-halving test sees it converge; the global step is never
        // clamped to the shortest segment (a 1 ps edge in a 4 us run would force 1e7 steps at level 0).
        let h_seg = h_target.min(len / (4.0 * refine));
        let mut hs: Vec<f64> = Vec::new();
        let (mut covered, mut g) = (0.0f64, h_first);
        while g < h_seg && covered + g < 0.5 * len {
            hs.push(g);
            covered += g;
            g *= GRADE_RATIO;
        }
        let rem = len - covered;
        let nu = ((rem / h_seg).ceil() as usize).max(2);
        hs.extend(std::iter::repeat_n(rem / nu as f64, nu));
        let steps = hs.len();
        let mut tn = s0;
        for (st, &h) in hs.iter().enumerate() {
            check_deadline()?;
            let tend = if st + 1 == steps { s1 } else { tn + h };
            let y = sys.step(tn, h, &x)?;
            if sys.ops > max_ops {
                return Err(
                    "work budget exceeded before the step-halving test converged".to_string(),
                );
            }
            while k < times.len() && times[k] <= tend {
                let th = ((times[k] - tn) / h).clamp(0.0, 1.0);
                let wts = dense_weights(th, st > 0);
                for (w, &u) in want.iter().enumerate() {
                    out[w][k] = if th <= 0.0 {
                        x[u]
                    } else {
                        wts[0] * x[u] + (0..3).map(|j| wts[j + 1] * y[j * n + u]).sum::<f64>()
                    };
                }
                k += 1;
            }
            x.copy_from_slice(&y[2 * n..3 * n]);
            tn = tend;
            if x.iter().any(|v| !v.is_finite()) {
                return Err("non-finite state".to_string());
            }
        }
    }
    while k < times.len() {
        for (w, &u) in want.iter().enumerate() {
            out[w][k] = x[u];
        }
        k += 1;
    }
    Ok(out)
}

/// Largest absolute difference between two refinement levels of one unknown.
fn level_diff(y: &[f64], prev: &[f64]) -> f64 {
    y.iter()
        .zip(prev)
        .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()))
}

/// Step-halving acceptance test for one unknown. Strict: `|y - prev| <= 1e-8 * swing + 1e-13 * max|y|`. With
/// `richardson`, also accepted when the difference is below `1e-6 * swing` AND at most a tenth of the previous
/// refinement's difference (observed order >= 3.3 of the order-5 method), so the geometric tail bounds the finest
/// level's error by `d * r / (1 - r) <= 0.12 d <= 1.2e-7 swing`. The integrator only falls back to the Richardson
/// level when the strict level is out of reach within the work budget (long lightly damped ringing); the corpus
/// tolerances on transients are >= 1e-4 of the swing.
fn converged(y: &[f64], prev: &[f64], prev_diff: Option<f64>, richardson: bool) -> bool {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in y {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    let amax = lo.abs().max(hi.abs());
    let d = level_diff(y, prev);
    let strict = 1e-8 * (hi - lo) + 1e-13 * amax + 1e-300;
    if d <= strict {
        return true;
    }
    let loose = 1e-6 * (hi - lo) + 1e-13 * amax;
    richardson && d <= loose && prev_diff.is_some_and(|p| d <= 0.1 * p)
}

/// Two consecutive refinement levels of every unknown for one (deck, axis), kept so that the several probes the
/// engine asks for on the same deck do not repeat the whole integration. Deterministic: a hit returns exactly what a
/// miss would have computed for the same probe.
struct TranCache {
    key: u64,
    prev: Vec<Vec<f64>>,
    last: Vec<Vec<f64>>,
    /// Per unknown: difference between `prev` and the level before it (`None` at the second level).
    prev_diff: Vec<Option<f64>>,
}

thread_local! {
    static TRAN_CACHE: std::cell::RefCell<Option<TranCache>> = const { std::cell::RefCell::new(None) };
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
}

fn tran_eval(net: &Net, probe: &str, axis: &[f64], deck: &str) -> Result<Vec<f64>, String> {
    let m = build_mna(net, false, true, TRAN_UNKNOWN_LIMIT)?;
    let pidx = probe_index(net, &m, probe, true)?;
    if !net
        .analyses
        .iter()
        .any(|a| matches!(a, Analysis::Tran { .. }))
    {
        return Err("deck has no .tran".to_string());
    }
    if axis.iter().any(|&t| t < 0.0) {
        return Err("negative time on axis".to_string());
    }
    // sort axis
    let mut order: Vec<usize> = (0..axis.len()).collect();
    order.sort_by(|&i, &j| {
        axis[i]
            .partial_cmp(&axis[j])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let times: Vec<f64> = order.iter().map(|&i| axis[i]).collect();
    let tmax = *times.last().unwrap();
    // initial state: DC operating point at t=0 (+ .ic forcing)
    let b0 = build_rhs(net, &m, &|_, s| s.wave_at(0.0));
    let x0 = if net.ic.is_empty() {
        solve_checked(m.n, &m.a, Some(&m.am), &b0)
            .map_err(|e| format!("DC operating point: {e}"))?
    } else {
        let n = m.n;
        let k = net.ic.len();
        let nt = n + k;
        let mut aa = vec![0.0; nt * nt];
        let mut aam = vec![0.0; nt * nt];
        for r in 0..n {
            aa[r * nt..r * nt + n].copy_from_slice(&m.a[r * n..(r + 1) * n]);
            aam[r * nt..r * nt + n].copy_from_slice(&m.am[r * n..(r + 1) * n]);
        }
        let mut bb = b0.clone();
        bb.resize(nt, 0.0);
        for (j, &(node, v)) in net.ic.iter().enumerate() {
            let col = n + j;
            aa[(node - 1) * nt + col] += 1.0; // forcing current into node
            aa[col * nt + (node - 1)] = 1.0;
            aam[(node - 1) * nt + col] += 1.0;
            aam[col * nt + (node - 1)] = 1.0;
            bb[col] = v;
        }
        let mut x = solve_checked(nt, &aa, Some(&aam), &bb)
            .map_err(|e| format!("DC operating point with .ic: {e}"))?;
        x.truncate(n);
        x
    };
    if tmax == 0.0 {
        let v = x0[pidx];
        return Ok(vec![v; axis.len()]);
    }
    // breakpoints
    let mut bp = vec![0.0, tmax];
    let mut fmax = 0.0f64;
    for el in &net.elems {
        if let Elem::V { src, .. } | Elem::I { src, .. } = el {
            src.breakpoints(tmax, &mut bp);
            if let Wave::Sin { freq, .. } = &src.wave {
                fmax = fmax.max(*freq);
            }
        }
    }
    bp.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut grid: Vec<f64> = Vec::new();
    for t in bp {
        if grid
            .last()
            .is_none_or(|&l| t - l > 1e-15 * tmax.max(1e-300))
        {
            grid.push(t);
        }
    }
    if grid.len() > 400_000 {
        return Err("too many source breakpoints".to_string());
    }
    // initial step
    let mut hmax = tmax / 16.0;
    if fmax > 0.0 {
        hmax = hmax.min(1.0 / (fmax * 24.0));
    }
    // Level-0 step: a quarter of the shortest source segment (the stiff modes a fast edge excites must be inside the
    // first level), but never below `tmax / 2048` (a 1 ps edge in a 4 us run would otherwise force 1e7 steps before
    // the first refinement); segments shorter than the step refine on their own ladder in `run_level`.
    let seglen_min = grid
        .windows(2)
        .map(|w| w[1] - w[0])
        .fold(f64::INFINITY, f64::min);
    hmax = hmax.min((seglen_min / 4.0).max(tmax / 2048.0));
    let a = radau_a();
    let ainv = inv3(&a);
    let rowsum = [
        ainv[0].iter().sum::<f64>(),
        ainv[1].iter().sum::<f64>(),
        ainv[2].iter().sum::<f64>(),
    ];
    let mut sys = TranSys {
        net,
        m,
        ainv,
        rowsum,
        cache: Vec::new(),
        ops: 0.0,
    };
    let max_ops = 2.5e10;
    let h_first = tmax * 1e-7;
    let mut key = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut key, deck.as_bytes());
    for t in axis {
        fnv(&mut key, &t.to_bits().to_le_bytes());
    }
    let unscramble = |y: &[f64]| {
        let mut out = vec![0.0; axis.len()];
        for (k, &i) in order.iter().enumerate() {
            out[i] = y[k];
        }
        out
    };
    // cache hit: the stored pair of levels already satisfies the acceptance test for this unknown
    let hit = TRAN_CACHE.with(|c| {
        c.borrow().as_ref().and_then(|c| {
            (c.key == key && converged(&c.last[pidx], &c.prev[pidx], c.prev_diff[pidx], true))
                .then(|| unscramble(&c.last[pidx]))
        })
    });
    if let Some(v) = hit {
        return Ok(v);
    }
    let all = sys.m.n.saturating_mul(times.len()) <= 4_000_000;
    let want: Vec<usize> = if all {
        (0..sys.m.n).collect()
    } else {
        vec![pidx]
    };
    let slot = if all { pidx } else { 0 };
    let store = |prev: Vec<Vec<f64>>, last: Vec<Vec<f64>>, prev_diff: Vec<Option<f64>>| {
        if all {
            TRAN_CACHE.with(|c| {
                *c.borrow_mut() = Some(TranCache {
                    key,
                    prev,
                    last,
                    prev_diff,
                })
            });
        }
    };
    let mut h = hmax;
    // levels k (prev) and k-1 (prev2), and the differences d_k, d_{k-1} of every wanted unknown
    let mut prev: Option<Vec<Vec<f64>>> = None;
    let mut prev2: Option<Vec<Vec<f64>>> = None;
    let mut d_k: Vec<Option<f64>> = vec![None; want.len()];
    let mut d_k1: Vec<Option<f64>> = vec![None; want.len()];
    for _level in 0..24 {
        let y = match run_level(
            &mut sys,
            &x0,
            &want,
            &times,
            &grid,
            (h, h_first),
            (hmax / h, max_ops),
        ) {
            Ok(y) => y,
            Err(e) => {
                // work budget exhausted: fall back to the finest completed level when its Richardson test holds
                if let (Some(p), Some(p2)) = (prev.as_ref(), prev2.as_ref()) {
                    if e.starts_with("work budget")
                        && converged(&p[slot], &p2[slot], d_k1[slot], true)
                    {
                        let out = unscramble(&p[slot]);
                        let (p, p2) = (
                            prev.take().unwrap_or_default(),
                            prev2.take().unwrap_or_default(),
                        );
                        store(p2, p, d_k1);
                        return Ok(out);
                    }
                }
                return Err(e);
            }
        };
        if let Some(p) = prev.as_ref() {
            if converged(&y[slot], &p[slot], d_k[slot], false) {
                let out = unscramble(&y[slot]);
                store(prev.take().unwrap_or_default(), y, d_k);
                return Ok(out);
            }
            for (w, d) in d_k1.iter_mut().enumerate() {
                *d = d_k[w];
            }
            for (w, d) in d_k.iter_mut().enumerate() {
                *d = Some(level_diff(&y[w], &p[w]));
            }
        }
        prev2 = prev.take();
        prev = Some(y);
        h *= 0.5;
    }
    Err("step-halving did not converge to 1e-8 of swing".to_string())
}

/// Black-box helpers for the `#[ignore]`d cross-validation tests (ngspice and `rsprobe` are only ever run as external
/// executables; their output is data, never an explanation source). Shared with `families.rs` and `fuzz.rs` tests.
#[cfg(test)]
pub mod testutil {
    use std::path::PathBuf;
    use std::process::Command;

    /// One plot of a binary rawfile.
    pub struct Raw {
        pub plot: String,
        pub names: Vec<String>,
        pub cols: Vec<Vec<f64>>,
    }

    impl Raw {
        /// Column by (lower-case) vector name; branch vectors `x#branch` are also found as `i(x)`.
        pub fn col(&self, name: &str) -> Option<&Vec<f64>> {
            let alt = name
                .strip_prefix("i(")
                .and_then(|r| r.strip_suffix(')'))
                .map(|n| format!("{n}#branch"));
            self.names
                .iter()
                .position(|n| n == name || Some(n) == alt.as_ref())
                .map(|i| &self.cols[i])
        }
    }

    pub fn work_dir() -> PathBuf {
        let d = std::env::var("H2_WORK")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
                    .join(".cache/h2-bench/work")
            });
        let _ = std::fs::create_dir_all(&d);
        d
    }

    pub fn ngspice_ok() -> bool {
        Command::new("ngspice")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Guarded command: systemd-run scope with memory cap if available, always `timeout`.
    pub fn guarded(secs: u32, mem_gib: u32) -> Command {
        let have = Command::new("systemd-run")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        let mut c;
        if have {
            c = Command::new("systemd-run");
            c.args([
                "--user",
                "--scope",
                "--quiet",
                "-p",
                &format!("MemoryMax={mem_gib}G"),
                "-p",
                "MemorySwapMax=1G",
            ]);
            c.arg("timeout").arg(format!("{secs}s"));
        } else {
            c = Command::new("timeout");
            c.arg(format!("{secs}s"));
        }
        c
    }

    /// Insert option text (e.g. `.options reltol=1e-9`) right after the title line.
    pub fn with_options(deck: &str, opts: &str) -> String {
        let mut it = deck.splitn(2, '\n');
        let title = it.next().unwrap_or("");
        format!("{title}\n{opts}\n{}", it.next().unwrap_or(""))
    }

    /// Tight DC/tran reference options; `.tran tstep tstop` gets `tmax = tstep/20` appended.
    pub fn tight(deck: &str) -> String {
        let mut out = Vec::new();
        for l in deck.lines() {
            let t: Vec<&str> = l.split_whitespace().collect();
            if t.len() == 3 && t[0].eq_ignore_ascii_case(".tran") {
                let tstep = super::parse_number(t[1]).unwrap_or(1e-9);
                out.push(format!(".tran {} {} 0 {:e}", t[1], t[2], tstep / 20.0));
            } else {
                out.push(l.to_string());
            }
        }
        with_options(
            &(out.join("\n") + "\n"),
            ".options reltol=1e-9 vntol=1e-12 abstol=1e-15",
        )
    }

    pub fn parse_raw(data: &[u8]) -> Result<Vec<Raw>, String> {
        let mut plots = Vec::new();
        let mut pos = 0;
        while pos < data.len() {
            let mut nvars = 0usize;
            let mut npts = 0usize;
            let mut plot = String::new();
            let mut names = Vec::new();
            loop {
                let e = data[pos..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .ok_or("truncated header")?
                    + pos;
                let line = String::from_utf8_lossy(&data[pos..e]).to_string();
                pos = e + 1;
                if line.starts_with("Binary:") {
                    break;
                }
                if let Some(v) = line.strip_prefix("No. Variables:") {
                    nvars = v.trim().parse().map_err(|_| "bad nvars")?;
                } else if let Some(v) = line.strip_prefix("No. Points:") {
                    npts = v.trim().parse().map_err(|_| "bad npts")?;
                } else if let Some(v) = line.strip_prefix("Plotname:") {
                    plot = v.trim().to_string();
                } else if line.starts_with("Variables:") {
                    for _ in 0..nvars {
                        let e = data[pos..]
                            .iter()
                            .position(|&b| b == b'\n')
                            .ok_or("truncated vars")?
                            + pos;
                        let l = String::from_utf8_lossy(&data[pos..e]).to_string();
                        pos = e + 1;
                        names.push(
                            l.split_whitespace()
                                .nth(1)
                                .unwrap_or("")
                                .to_ascii_lowercase(),
                        );
                    }
                }
            }
            let need = nvars * npts * 8;
            if pos + need > data.len() {
                return Err("truncated data".to_string());
            }
            let mut cols = vec![Vec::with_capacity(npts); nvars];
            for p in 0..npts {
                for (v, col) in cols.iter_mut().enumerate() {
                    let o = pos + (p * nvars + v) * 8;
                    col.push(f64::from_le_bytes(data[o..o + 8].try_into().unwrap()));
                }
            }
            pos += need;
            plots.push(Raw { plot, names, cols });
        }
        Ok(plots)
    }

    /// Run `ngspice -b -r` on `deck` (as given) and return the last plot.
    pub fn run_ngspice(deck: &str, tag: &str) -> Result<Raw, String> {
        let dir = work_dir();
        let sp = dir.join(format!("{tag}.sp"));
        let raw = dir.join(format!("{tag}.raw"));
        let _ = std::fs::remove_file(&raw);
        std::fs::write(&sp, deck).map_err(|e| e.to_string())?;
        let out = guarded(60, 4)
            .arg("ngspice")
            .arg("-b")
            .arg("-r")
            .arg(&raw)
            .arg(&sp)
            .output()
            .map_err(|e| e.to_string())?;
        let log = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        for bad in [
            "singular matrix",
            "no convergence",
            "Timestep too small",
            "Too many iterations",
            "Error:",
        ] {
            if log.contains(bad) {
                return Err(format!("ngspice: {bad}"));
            }
        }
        let data = std::fs::read(&raw).map_err(|e| format!("no rawfile ({e})"))?;
        let mut plots = parse_raw(&data)?;
        plots.pop().ok_or_else(|| "empty rawfile".to_string())
    }

    /// Run the RustSpice probe (`RSPROBE` env var) and return (exit ok, stdout, last stderr line).
    pub fn run_probe(deck_path: &std::path::Path) -> Option<(bool, String, String)> {
        let bin = std::env::var("RSPROBE").ok()?;
        let secs: u32 = std::env::var("H2_PROBE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        let out = guarded(secs, 4)
            .arg(bin)
            .arg(deck_path)
            .arg("--max-tran-rows")
            .arg("1000000")
            .output()
            .ok()?;
        let err = String::from_utf8_lossy(&out.stderr)
            .lines()
            .last()
            .unwrap_or("")
            .to_string();
        Some((
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            err,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(deck: &str, spec: &str, probe: &str, axis: &[f64]) -> Vec<f64> {
        evaluate(deck, spec, probe, axis).unwrap_or_else(|e| panic!("oracle error: {e}"))
    }

    #[test]
    fn numbers_and_suffixes() {
        let cases = [
            ("1k", 1e3),
            ("2.5meg", 2.5e6),
            ("1MEG", 1e6),
            ("3m", 3e-3),
            ("4u", 4e-6),
            ("5n", 5e-9),
            ("6p", 6e-12),
            ("7f", 7e-15),
            ("8g", 8e9),
            ("9t", 9e12),
            ("10mil", 254e-6),
            ("1e3", 1e3),
            ("1.5e-3k", 1.5),
            ("-2.5", -2.5),
            ("10ohm", 10.0),
            ("5v", 5.0),
            ("1.5kohm", 1500.0),
            (".5", 0.5),
        ];
        for (s, v) in cases {
            let got = parse_number(s).unwrap_or_else(|| panic!("no parse {s}"));
            assert!(
                (got - v).abs() <= 1e-15 * v.abs().max(1.0),
                "{s}: {got} vs {v}"
            );
        }
        assert!(parse_number("abc").is_none());
        assert!(parse_number("").is_none());
        assert!(parse_number("1.2.3").is_none());
    }

    #[test]
    fn expressions_and_params() {
        let mut p = BTreeMap::new();
        p.insert("rv".to_string(), 2.0);
        assert_eq!(eval_value("{2*rv}", &p).unwrap(), 4.0);
        assert_eq!(eval_value("'rv+1k'", &p).unwrap(), 1002.0);
        assert!((eval_value("{(1+2)**2/3}", &p).unwrap() - 3.0).abs() < 1e-15);
        assert!(eval_value("{nosuch}", &p).is_err());
        assert!(eval_value("{1/0}", &p).is_err());
    }

    #[test]
    fn thermal_voltage_matches_ngspice_observation() {
        // Observed via ngspice-47 (black box): 1 mA into IS=1e-14, N=1 diode => Vt = 0.025864917007157... V
        assert!((VT - 0.025_864_917_007_157_463).abs() < 3e-15, "VT = {VT}");
    }

    #[test]
    fn radau_constants_satisfy_order_conditions() {
        let a = radau_a();
        for i in 0..3 {
            let rs: f64 = a[i].iter().sum();
            assert!((rs - RADAU_C[i]).abs() < 1e-15, "row sum {i}");
        }
        // C(3): sum_j a_ij c_j^(k-1) = c_i^k / k for k = 1..3 ; B(5): last row (b) sums
        for i in 0..3 {
            for k in 1..=3 {
                let s: f64 = (0..3).map(|j| a[i][j] * RADAU_C[j].powi(k - 1)).sum();
                let e = RADAU_C[i].powi(k) / k as f64;
                assert!((s - e).abs() < 1e-15, "C({k}) row {i}: {s} vs {e}");
            }
        }
        for k in 1..=5 {
            let s: f64 = (0..3).map(|j| a[2][j] * RADAU_C[j].powi(k - 1)).sum();
            assert!((s - 1.0 / k as f64).abs() < 1e-15, "B({k})");
        }
        let ai = inv3(&a);
        for i in 0..3 {
            for j in 0..3 {
                let s: f64 = (0..3).map(|k| a[i][k] * ai[k][j]).sum();
                assert!((s - if i == j { 1.0 } else { 0.0 }).abs() < 1e-13);
            }
        }
        let s6 = 6.0f64.sqrt();
        assert!((RADAU_C[0] - (4.0 - s6) / 10.0).abs() < 1e-16);
        assert!((RADAU_C[1] - (4.0 + s6) / 10.0).abs() < 1e-16);
    }

    #[test]
    fn dense_lu_random_matches_known_solution() {
        // deterministic pseudo-random well-conditioned system
        let n = 40;
        let mut s = 0x2545_F491_4F6C_DD1Du64;
        let mut rnd = || {
            s ^= s >> 12;
            s ^= s << 25;
            s ^= s >> 27;
            (s.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        let mut a = vec![0.0; n * n];
        for v in a.iter_mut() {
            *v = rnd();
        }
        for i in 0..n {
            a[i * n + i] += 3.0;
        }
        let xt: Vec<f64> = (0..n).map(|i| (i as f64).sin()).collect();
        let b: Vec<f64> = (0..n).map(|i| dot2(&a[i * n..(i + 1) * n], &xt)).collect();
        let x = solve_dense_refined(n, &a, &b).unwrap();
        for i in 0..n {
            assert!((x[i] - xt[i]).abs() < 1e-14, "{i}: {} vs {}", x[i], xt[i]);
        }
        // singular
        let mut sing = vec![1.0; 4];
        sing[3] = 1.0;
        assert!(solve_dense_refined(2, &sing, &[1.0, 1.0]).is_err());
    }

    #[test]
    fn dc_resistor_divider_closed_form() {
        let deck = "divider\n.param rtop=2k rbot=1k\nV1 in 0 DC 9\nR1 in mid {rtop}\nR2 mid 0 {rbot}\n.op\n.end\n";
        let v = ev(deck, "dc_linear", "v(mid)", &[0.0]);
        assert!((v[0] - 3.0).abs() < 1e-14, "{v:?}");
        let i = ev(deck, "dc_linear", "i(v1)", &[0.0]);
        assert!((i[0] + 3e-3).abs() < 1e-16, "{i:?}"); // current into + terminal is negative when sourcing
        assert!(evaluate(deck, "dc_linear", "v(nosuch)", &[0.0]).is_err());
        assert!(evaluate(deck, "bogus", "v(mid)", &[0.0]).is_err());
    }

    #[test]
    fn dc_linear_sources_and_vccs_closed_form() {
        // I1 pushes 1 mA into node a (from ground) through source; G draws gm*v(a) from node b
        let deck = "t\nI1 0 a 1m\nR1 a 0 1k\nG1 b 0 a 0 2m\nR2 b 0 500\n.op\n.end\n";
        let va = ev(deck, "dc_linear", "v(a)", &[0.0])[0];
        assert!((va - 1.0).abs() < 1e-14);
        // G: current gm*v(a) flows from node b through the source to ground => KCL at b: v(b)/500 + gm*va = 0
        let vb = ev(deck, "dc_linear", "v(b)", &[0.0])[0];
        assert!((vb + 1.0).abs() < 1e-13, "{vb}");
    }

    #[test]
    fn dc_linear_sweep_and_unsupported() {
        let deck = "t\nV1 in 0 0\nR1 in out 1k\nR2 out 0 1k\n.dc V1 0 4 1\n.end\n";
        let v = ev(deck, "dc_linear", "v(out)", &[0.0, 1.0, 2.0, 3.0, 4.0]);
        for (k, x) in v.iter().enumerate() {
            assert!((x - k as f64 / 2.0).abs() < 1e-15);
        }
        let bad = "t\nM1 d g s b nmos\n.op\n";
        assert!(evaluate(bad, "dc_linear", "v(d)", &[0.0]).is_err());
        let sing = "t\nV1 a 0 1\nR1 b c 1k\n.op\n";
        assert!(
            evaluate(sing, "dc_linear", "v(b)", &[0.0]).is_err(),
            "floating island must be rejected"
        );
        let dio = "t\n.model dm D(IS=1e-14)\nV1 a 0 1\nD1 a 0 dm\n.op\n";
        assert!(evaluate(dio, "dc_linear", "v(a)", &[0.0]).is_err());
        let big = format!(
            "t\nV1 n0 0 1\n{}\n.op\n",
            (0..2100)
                .map(|i| format!("R{i} n{i} n{} 1", i + 1))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert!(evaluate(&big, "dc_linear", "v(n5)", &[0.0])
            .unwrap_err()
            .contains("dense"));
    }

    #[test]
    fn op_and_dc_in_one_deck_are_told_apart_by_the_axis() {
        let deck = "t\nV1 in 0 3\nR1 in a 1k\nR2 a 0 1k\n.op\n.dc V1 0 4 2\n.end\n";
        assert!((ev(deck, "dc_linear", "v(a)", &[0.0])[0] - 1.5).abs() < 1e-15);
        let sweep = ev(deck, "dc_linear", "v(a)", &[0.0, 2.0, 4.0]);
        assert!((sweep[1] - 1.0).abs() < 1e-15 && (sweep[2] - 2.0).abs() < 1e-15);
        assert!(evaluate(
            "t\nV1 in 0 3\nR1 in 0 1k\n.op\n",
            "dc_linear",
            "v(in)",
            &[0.0, 1.0]
        )
        .is_err());
    }

    fn bracket_diode(vs: f64, r: f64, is: f64, n: f64, gmin: f64) -> f64 {
        // solve (vs - vd)/r = is(exp(vd/nVt)-1) + gmin*vd by bisection on [0, vs]
        let f = |vd: f64| (vs - vd) / r - (is * ((vd / (n * VT)).exp() - 1.0) + gmin * vd);
        let (mut lo, mut hi) = (0.0f64, vs.max(0.1));
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }

    #[test]
    fn dc_diode_resistor_matches_bracketed_scalar_solve() {
        for (vs, r, is, n) in [
            (5.0, 1e3, 1e-14, 1.0),
            (1.0, 10.0, 1e-12, 1.5),
            (12.0, 1e5, 1e-16, 1.0),
        ] {
            let deck = format!(
                "d\n.model dm D(IS={is:e} N={n})\nV1 in 0 DC {vs}\nR1 in a {r}\nD1 a 0 dm\n.op\n.end\n"
            );
            let va = ev(&deck, "dc_nonlinear", "v(a)", &[0.0])[0];
            let exact = bracket_diode(vs, r, is, n, 1e-12);
            assert!((va - exact).abs() < 1e-12, "vs={vs} r={r}: {va} vs {exact}");
            let vg = ev(&deck, "dc_nonlinear(gmin=1e-9)", "v(a)", &[0.0])[0];
            let exg = bracket_diode(vs, r, is, n, 1e-9);
            assert!((vg - exg).abs() < 1e-12, "gmin: {vg} vs {exg}");
        }
    }

    #[test]
    fn dc_diode_sweep_continuation_and_reverse() {
        let deck =
            "d\n.model dm D(IS=1e-14)\nV1 in 0 0\nR1 in a 100\nD1 a 0 dm\n.dc V1 -5 5 0.5\n.end\n";
        let axis: Vec<f64> = (0..21).map(|k| -5.0 + 0.5 * k as f64).collect();
        let v = ev(deck, "dc_nonlinear", "v(a)", &axis);
        for (k, &vs) in axis.iter().enumerate() {
            // bisection on [-vs.abs()-1, vs.abs()+1]
            let f = |vd: f64| (vs - vd) / 100.0 - (1e-14 * ((vd / VT).exp() - 1.0) + 1e-12 * vd);
            let (mut lo, mut hi) = (-10.0f64, 10.0f64);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if f(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            assert!(
                (v[k] - 0.5 * (lo + hi)).abs() < 1e-12,
                "k={k} vs={vs}: {} vs {}",
                v[k],
                0.5 * (lo + hi)
            );
        }
    }

    #[test]
    fn dc_stacked_diodes_converge() {
        let n = 6;
        let mut deck = String::from("chain\n.model dm D(IS=1e-14)\nV1 in 0 DC 10\nR1 in d0 1k\n");
        for k in 0..n {
            let nxt = if k + 1 == n {
                "0".to_string()
            } else {
                format!("d{}", k + 1)
            };
            deck.push_str(&format!("D{k} d{k} {nxt} dm\n"));
        }
        deck.push_str(".op\n.end\n");
        let v0 = ev(&deck, "dc_nonlinear", "v(d0)", &[0.0])[0];
        // current from the single-diode law applied to the stack of n identical diodes: v0 = n*vd with vd solving
        // (10 - n vd)/1k = Is(exp(vd/Vt)-1)+gmin*vd
        let f =
            |vd: f64| (10.0 - n as f64 * vd) / 1e3 - (1e-14 * ((vd / VT).exp() - 1.0) + 1e-12 * vd);
        let (mut lo, mut hi) = (0.0f64, 10.0 / n as f64);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let exact = n as f64 * 0.5 * (lo + hi);
        assert!((v0 - exact).abs() < 1e-11, "{v0} vs {exact}");
    }

    // ---- transient closed forms

    /// Response of a unit-DC-gain system to a unit trapezoid ramp 0 -> 1 over [0, tr] given the integral of its step
    /// response `sint(t)` (zero for t <= 0).
    fn ramp_response(sint: &dyn Fn(f64) -> f64, t: f64, tr: f64) -> f64 {
        (sint(t) - if t > tr { sint(t - tr) } else { 0.0 }) / tr
    }

    fn pwl_deck(body: &str, tr: f64, tstop: f64) -> String {
        format!(
            "tran test\nV1 in 0 PWL(0 0 {tr:e} 1 1 1)\n{body}.tran {:e} {tstop:e}\n.end\n",
            tstop / 100.0
        )
    }

    #[test]
    fn tran_rc_ramp_closed_form() {
        let (r, c, tr) = (1e3, 1e-9, 0.5e-6);
        let tau = r * c;
        let deck = pwl_deck("R1 in out 1k\nC1 out 0 1n\n", tr, 5e-6);
        let axis: Vec<f64> = (0..=50).map(|k| k as f64 * 0.1e-6).collect();
        // step response 1 - e^{-t/tau}; integral t - tau(1 - e^{-t/tau})
        let sint = |t: f64| {
            if t <= 0.0 {
                0.0
            } else {
                t - tau * (1.0 - (-t / tau).exp())
            }
        };
        let v = ev(&deck, "tran_lti", "v(out)", &axis);
        for (k, &t) in axis.iter().enumerate() {
            let ex = ramp_response(&sint, t, tr);
            assert!((v[k] - ex).abs() < 2e-9, "t={t}: {} vs {ex}", v[k]);
        }
    }

    #[test]
    fn tran_rlc_all_damping_regimes_and_lc() {
        let tr = 1e-6;
        // series V -> R -> L -> out(C to ground); vC response, w0^2 = 1/LC, alpha = R/2L
        let l = 1e-3;
        let c = 1e-6;
        let w0 = (1.0f64 / (l * c)).sqrt();
        let tstop = 2e-3;
        let axis: Vec<f64> = (0..=100).map(|k| k as f64 * tstop / 100.0).collect();
        for (r, regime) in [
            (2.0, "under"),
            (63.245_553_203_367_58, "crit"),
            (400.0, "over"),
            (0.0, "lc"),
        ] {
            let body = if r > 0.0 {
                format!("R1 in a {r:e}\nL1 a out {l:e}\nC1 out 0 {c:e}\n")
            } else {
                format!("L1 in out {l:e}\nC1 out 0 {c:e}\n")
            };
            let deck = pwl_deck(&body, tr, tstop);
            let alpha = r / (2.0 * l);
            let sint = |t: f64| -> f64 {
                if t <= 0.0 {
                    return 0.0;
                }
                match regime {
                    "lc" => t - (w0 * t).sin() / w0,
                    "under" => {
                        let wd = (w0 * w0 - alpha * alpha).sqrt();
                        let den = alpha * alpha + wd * wd;
                        let ic = ((-alpha * t).exp()
                            * (wd * (wd * t).sin() - alpha * (wd * t).cos())
                            + alpha)
                            / den;
                        let is = (wd
                            - (-alpha * t).exp() * (alpha * (wd * t).sin() + wd * (wd * t).cos()))
                            / den;
                        t - ic - alpha / wd * is
                    }
                    "crit" => {
                        let a = alpha;
                        let e = (-a * t).exp();
                        t - ((1.0 - e) / a + a * (1.0 - e * (1.0 + a * t)) / (a * a))
                    }
                    _ => {
                        let d = (alpha * alpha - w0 * w0).sqrt();
                        let (s1, s2) = (-alpha + d, -alpha - d);
                        // s(t) = 1 - (s2 e^{s1 t} - s1 e^{s2 t})/(s2 - s1)
                        let i1 = ((s1 * t).exp() - 1.0) / s1;
                        let i2 = ((s2 * t).exp() - 1.0) / s2;
                        t - (s2 * i1 - s1 * i2) / (s2 - s1)
                    }
                }
            };
            let v = ev(&deck, "tran_lti", "v(out)", &axis);
            let mut worst = 0.0f64;
            for (k, &t) in axis.iter().enumerate() {
                let ex = ramp_response(&sint, t, tr);
                worst = worst.max((v[k] - ex).abs());
            }
            assert!(worst < 5e-9, "{regime}: worst err {worst:e}");
        }
    }

    #[test]
    fn tran_rc_discharge_with_ic_and_sine_source() {
        // .ic forces v(out)=1 in the operating point; capacitor then discharges through R: exp(-t/RC)
        let deck = "rc\nR1 out 0 1k\nC1 out 0 1u\n.ic v(out)=1\n.tran 1u 5m\n.end\n";
        let axis: Vec<f64> = (0..=50).map(|k| k as f64 * 1e-4).collect();
        let v = ev(deck, "tran_lti", "v(out)", &axis);
        for (k, &t) in axis.iter().enumerate() {
            assert!((v[k] - (-t / 1e-3).exp()).abs() < 1e-9, "t={t}");
        }
        // RC low-pass driven by a sine (V0*sin(wt)) from the zero state: closed-form steady + transient
        let (r, c, f0, va) = (1e3f64, 1e-6f64, 300.0f64, 2.0f64);
        let tau = r * c;
        let w = 2.0 * std::f64::consts::PI * f0;
        let deck = format!(
            "rc\nV1 in 0 SIN(0 {va} {f0})\nR1 in out {r}\nC1 out 0 {c}\n.tran 1u 10m\n.end\n"
        );
        let axis: Vec<f64> = (0..=100).map(|k| k as f64 * 1e-4).collect();
        let v = ev(&deck, "tran_lti", "v(out)", &axis);
        let k = w * tau;
        for (i, &t) in axis.iter().enumerate() {
            let ex =
                va / (1.0 + k * k) * ((w * t).sin() - k * (w * t).cos() + k * (-t / tau).exp());
            assert!((v[i] - ex).abs() < 1e-8, "t={t}: {} vs {ex}", v[i]);
        }
    }

    #[test]
    fn tran_pulse_breakpoints_and_current_probe() {
        // RC with PULSE: check the branch current sign convention and the settled value
        let deck = "p\nV1 in 0 PULSE(0 1 1u 10n 10n 5u 20u)\nR1 in out 100\nC1 out 0 1n\n.tran 10n 40u\n.end\n";
        let axis = [0.0, 0.5e-6, 4e-6, 6.5e-6, 12e-6, 24e-6];
        let v = ev(deck, "tran_lti", "v(out)", &axis);
        let i = ev(deck, "tran_lti", "i(v1)", &axis);
        assert!(v[0].abs() < 1e-14 && v[1].abs() < 1e-14);
        assert!((v[2] - 1.0).abs() < 1e-6, "settled high {}", v[2]);
        assert!(i[2].abs() < 1e-6); // (1 - v)/R tiny
        assert!(v[4].abs() < 1e-6, "settled low");
        // current into + terminal is -(current sourced into the circuit): negative while charging
        let early = ev(deck, "tran_lti", "i(v1)", &[1.02e-6]);
        assert!(early[0] < 0.0, "charging current {}", early[0]);
        // second pulse (period 20us) starts at 21us: value at 24us high again
        assert!((v[5] - 1.0).abs() < 1e-6, "second pulse");
    }

    #[test]
    fn tran_rejects_out_of_subset() {
        let base = |s: &str| format!("t\n{s}\nR1 in 0 1k\nC1 in 0 1n\n.tran 1n 1u\n.end\n");
        assert!(evaluate(
            &base("V1 in 0 PULSE(0 1 0 0 0 1u 2u)"),
            "tran_lti",
            "v(in)",
            &[1e-7]
        )
        .is_err());
        assert!(evaluate(
            &base("V1 in 0 EXP(0 1 0 1n 1n 1n)"),
            "tran_lti",
            "v(in)",
            &[1e-7]
        )
        .is_err());
        assert!(evaluate(
            &base("V1 in 0 DC 1 SIN(0.5 1 1e6)"),
            "tran_lti",
            "v(in)",
            &[1e-7]
        )
        .is_err());
        assert!(evaluate(
            "t\nV1 in 0 1\nR1 in 0 1k\nC1 in 0 1n\n.tran 1n 1u uic\n",
            "tran_lti",
            "v(in)",
            &[1e-7]
        )
        .is_err());
        let many: String = (0..450)
            .map(|i| format!("R{i} n{i} n{} 1\n", i + 1))
            .collect();
        let deck = format!("t\nV1 n0 0 1\n{many}C1 n450 0 1p\n.tran 1n 1u\n");
        assert!(evaluate(&deck, "tran_lti", "v(n1)", &[1e-7])
            .unwrap_err()
            .contains("limit"));
        // probe not present
        assert!(evaluate(&base("V1 in 0 1"), "tran_lti", "v(zz)", &[1e-7]).is_err());
        assert!(evaluate(&base("V1 in 0 1"), "tran_lti", "v(in)", &[]).is_err());
    }

    #[test]
    fn tran_is_deterministic_and_axis_order_independent() {
        let deck = "p\nV1 in 0 PULSE(0 1 1u 10n 10n 5u 20u)\nR1 in out 100\nC1 out 0 1n\nL1 out m 1u\nR2 m 0 50\n.tran 10n 40u\n.end\n";
        let axis = [3e-6, 1e-6, 7e-6, 2.5e-6];
        let a = ev(deck, "tran_lti", "v(m)", &axis);
        let b = ev(deck, "tran_lti", "v(m)", &axis);
        assert_eq!(a, b);
        let sorted = [1e-6, 2.5e-6, 3e-6, 7e-6];
        let c = ev(deck, "tran_lti", "v(m)", &sorted);
        assert_eq!(a[1], c[0]);
        assert_eq!(a[3], c[1]);
        assert_eq!(a[0], c[2]);
        assert_eq!(a[2], c[3]);
    }

    #[test]
    fn ladder_matches_high_precision_reference_values() {
        // 50-digit mpmath evaluation of the impedance recursion Z_n = rsh, Z_k = rsh || (rs + Z_{k+1}),
        // v_k = v_{k-1} Z_k / (rs + Z_k): a different algorithm from the closed form under test
        type Case = (usize, f64, [(usize, f64); 4], f64);
        let cases: [Case; 3] = [
            (
                1_000_000,
                4e10,
                [
                    (250_000, 0.286650243557521),
                    (500_000, 0.08263432865489075),
                    (750_000, 0.02544703505226711),
                    (1_000_000, 0.01347524853631213),
                ],
                -4.999533523598379e-6,
            ),
            (
                10_000,
                4e6,
                [
                    (2_500, 0.2866501753255567),
                    (5_000, 0.08263405899626305),
                    (7_500, 0.02544608137777824),
                    (10_000, 0.0134719152501693),
                ],
                -0.0004998296404423836,
            ),
            (
                1_000,
                4e4,
                [
                    (250, 0.2866498932334435),
                    (500, 0.08263180973254423),
                    (750, 0.0254375270795472),
                    (1_000, 0.01344175117690651),
                ],
                -0.004987063904295299,
            ),
        ];
        for (n, rsh, nodes, i1) in cases {
            let spec = format!("ladder(n={n},rs=1,rsh={rsh:e})");
            for (k, want) in nodes {
                let got = ev("unused", &spec, &format!("v(n{k})"), &[0.0])[0];
                assert!(
                    (got - want).abs() <= 2e-13 * want,
                    "n={n} v(n{k}): {got:e} vs {want:e}"
                );
            }
            let got = ev("unused", &spec, "i(v1)", &[0.0, 1.0])[1];
            assert!(
                (got - i1).abs() <= 2e-13 * i1.abs(),
                "n={n} i(v1): {got:e} vs {i1:e}"
            );
        }
    }

    #[test]
    fn ladder_agrees_with_dense_lu_of_the_netlist() {
        for (n, rs, rsh, vin) in [
            (60usize, 2.5, 900.0, 3.0),
            (200, 1.0, 1.6e3, 1.0),
            (8, 10.0, 7.0, -2.0),
        ] {
            let mut deck = format!("ladder\nV1 n0 0 DC {vin}\n");
            for k in 1..=n {
                deck.push_str(&format!("Rs{k} n{} n{k} {rs}\nRp{k} n{k} 0 {rsh}\n", k - 1));
            }
            deck.push_str(".op\n.end\n");
            let spec = format!("ladder(n={n},rs={rs},rsh={rsh},v={vin})");
            for probe in [
                "v(n0)".to_string(),
                "v(n1)".to_string(),
                format!("v(n{})", n / 2),
                format!("v(n{n})"),
                "i(v1)".to_string(),
            ] {
                let dense = ev(&deck, "dc_linear", &probe, &[0.0])[0];
                let closed = ev("unused", &spec, &probe, &[0.0])[0];
                // the dense solve sees conductances rounded to doubles (cond ~ 1e4 here): ~1e-12 relative is inherent, the
                // closed form matches a 50-digit evaluation to 8e-16 (n = 200, rsh = 1600, v(n200))
                assert!(
                    (dense - closed).abs() <= 1e-11 * dense.abs().max(1e-3),
                    "n={n} {probe}: dense {dense:e} vs closed {closed:e}"
                );
            }
        }
        assert!(evaluate("x", "ladder(n=10,rs=1)", "v(n1)", &[0.0]).is_err());
        assert!(evaluate("x", "ladder(n=10,rs=1,rsh=5)", "v(n11)", &[0.0]).is_err());
        assert!(evaluate("x", "ladder(n=0,rs=1,rsh=5)", "v(n1)", &[0.0]).is_err());
        assert!(evaluate("x", "ladder(n=10,rs=-1,rsh=5)", "v(n1)", &[0.0]).is_err());
    }

    #[test]
    fn dc_condition_matches_numpy_style_hand_values() {
        // V1 n1 0 1; R1 n1 n2 1; R2 n2 0 1: unknowns [v(n1), v(n2), i(v1)],
        // A = [[1,-1,1],[-1,2,0],[1,0,0]]; inverse = [[0,0,1],[0,0.5,0.5],[1,0.5,-0.5]] (checked by hand: A * inv = I)
        // |A|_1 = max(3,3,1) = 3, |A^-1|_1 = max(1,1,2) = 2 -> cond 6
        let c = dc_condition("t\nV1 n1 0 1\nR1 n1 n2 1\nR2 n2 0 1\n.op\n.end\n").unwrap();
        assert!((c - 6.0).abs() < 1e-12, "{c}");
        // a singular network (two sources fixing the same node) is an error, not a number
        assert!(dc_condition("t\nV1 n1 0 1\nV2 n1 0 2\nR1 n1 0 1\n.op\n.end\n").is_err());
    }

    #[test]
    fn dc_cache_returns_what_a_miss_computes() {
        let deck = "t\n.model dm D(IS=1e-14 N=1)\nV1 in 0 0\nR1 in a 1k\nD1 a b dm\nD2 b c dm\nR2 c 0 220\n.dc V1 0 4 0.5\n.end\n";
        let axis: Vec<f64> = (0..=8).map(|k| 0.5 * k as f64).collect();
        let first = ev(deck, "dc_nonlinear", "v(b)", &axis);
        let again = ev(deck, "dc_nonlinear", "v(b)", &axis);
        let other = ev(deck, "dc_nonlinear", "v(a)", &axis);
        let cold = ev(deck, "dc_nonlinear", "v(b)", &axis);
        assert_eq!(first, again);
        assert_eq!(first, cold);
        assert!(other.iter().zip(&first).any(|(a, b)| a != b));
    }

    #[test]
    fn tran_high_q_ringing_matches_closed_form() {
        // Q = 60 series RLC, 20 periods: the strict 1e-8 step-halving level is expensive for ringing, so this also
        // covers the Richardson acceptance; closed form of the ramp response as in tran_rlc_all_damping_regimes_and_lc
        let (l, c, tr) = (1e-3f64, 1e-6f64, 1e-6f64);
        let w0 = (1.0 / (l * c)).sqrt();
        let r = (l / c).sqrt() / 60.0;
        let alpha = r / (2.0 * l);
        let wd = (w0 * w0 - alpha * alpha).sqrt();
        let tstop = 20.0 * 2.0 * std::f64::consts::PI / wd;
        let sint = |t: f64| -> f64 {
            if t <= 0.0 {
                return 0.0;
            }
            let den = alpha * alpha + wd * wd;
            let ic =
                ((-alpha * t).exp() * (wd * (wd * t).sin() - alpha * (wd * t).cos()) + alpha) / den;
            let is =
                (wd - (-alpha * t).exp() * (alpha * (wd * t).sin() + wd * (wd * t).cos())) / den;
            t - ic - alpha / wd * is
        };
        let deck = pwl_deck(
            &format!("R1 in a {r:e}\nL1 a out {l:e}\nC1 out 0 {c:e}\n"),
            tr,
            tstop,
        );
        let axis: Vec<f64> = (0..=200).map(|k| k as f64 * tstop / 200.0).collect();
        let v = ev(&deck, "tran_lti", "v(out)", &axis);
        let worst = axis.iter().zip(&v).fold(0.0f64, |m, (&t, &y)| {
            m.max((y - ramp_response(&sint, t, tr)).abs())
        });
        assert!(worst < 5e-7, "worst err {worst:e}");
    }

    /// Cross-validation against ngspice-47 run as a black box with tight tolerances. Run by hand:
    /// `cargo test -p rustspice-cli oracle::tests::dump_and_check -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_and_check() {
        use super::testutil::*;
        if !ngspice_ok() {
            eprintln!("ngspice not available: skipped");
            return;
        }
        struct Case {
            id: &'static str,
            spec: &'static str,
            deck: &'static str,
            probes: &'static [&'static str],
        }
        let cases = [
            Case {
                id: "dc_div_param",
                spec: "dc_linear",
                deck: "t\n.param rt=2k rb=1.5k\nV1 in 0 DC 9\nR1 in mid {rt}\nR2 mid 0 {rb}\nI1 0 mid 1m\nG1 x 0 mid 0 1.5m\nR3 x 0 3.3k\n.op\n.end\n",
                probes: &["v(mid)", "v(x)", "i(v1)"],
            },
            Case {
                id: "dc_gnd_alias",
                spec: "dc_linear",
                deck: "t\nV1 in gnd DC 5\nR1 in a 1k\nR2 a gnd 2k\n.op\n.end\n",
                probes: &["v(a)", "i(v1)"],
            },
            Case {
                id: "dc_wide_range",
                spec: "dc_linear",
                deck: "t\nV1 a 0 3\nR1 a b 0.1\nR2 b c 1e7\nR3 c 0 100\nR4 b 0 2.2meg\nI1 0 c 1u\n.op\n.end\n",
                probes: &["v(b)", "v(c)", "i(v1)"],
            },
            Case {
                id: "dc_sweep_lin",
                spec: "dc_linear",
                deck: "t\nV1 in 0 0\nR1 in a 1k\nR2 a 0 3k\n.dc V1 -2 2 0.25\n.end\n",
                probes: &["v(a)", "i(v1)"],
            },
            Case {
                id: "dcnl_diode_r",
                spec: "dc_nonlinear",
                deck: "t\n.model dm D(IS=2e-15 N=1.2)\nV1 in 0 5\nR1 in a 470\nD1 a b dm\nR2 b 0 100\n.op\n.end\n",
                probes: &["v(a)", "v(b)", "i(v1)"],
            },
            Case {
                id: "dcnl_stack_sweep",
                spec: "dc_nonlinear",
                deck: "t\n.model dm D(IS=1e-14 N=1)\nV1 in 0 0\nR1 in a 1k\nD1 a b dm\nD2 b c dm\nR2 c 0 220\n.dc V1 0 4 0.1\n.end\n",
                probes: &["v(a)", "v(b)", "i(v1)"],
            },
            Case {
                id: "dcnl_reverse",
                spec: "dc_nonlinear",
                deck: "t\n.model dm D(IS=1e-14 N=1)\nV1 in 0 -3\nR1 in a 1k\nD1 a 0 dm\n.op\n.end\n",
                probes: &["v(a)"],
            },
            Case {
                id: "tran_rc_pulse",
                spec: "tran_lti",
                deck: "t\nV1 in 0 PULSE(0 1 1n 0.5n 0.5n 4n 10n)\nR1 in out 100\nC1 out 0 2p\n.tran 0.1n 30n\n.end\n",
                probes: &["v(out)", "i(v1)"],
            },
            Case {
                id: "tran_rlc_pwl",
                spec: "tran_lti",
                deck: "t\nV1 in 0 PWL(0 0 1n 1 20n 1 21n -1 50n -1)\nR1 in a 5\nL1 a out 10n\nC1 out 0 10p\nR2 out 0 100\n.tran 0.1n 40n\n.end\n",
                probes: &["v(out)", "i(l1)", "i(v1)"],
            },
            Case {
                id: "tran_sin_g",
                spec: "tran_lti",
                deck: "t\nV1 in 0 SIN(0.5 1 50meg 4n)\nR1 in a 50\nC1 a 0 4p\nG1 b 0 a 0 2m\nR2 b 0 1k\nC2 b 0 1p\n.tran 0.2n 100n\n.end\n",
                probes: &["v(a)", "v(b)"],
            },
            Case {
                id: "tran_ic",
                spec: "tran_lti",
                deck: "t\nR1 out 0 1k\nC1 out 0 1n\nR2 out x 2k\nC2 x 0 2n\n.ic v(out)=2\n.tran 10n 10u\n.end\n",
                probes: &["v(out)", "v(x)"],
            },
        ];
        let mut worst = 0.0f64;
        for c in &cases {
            let ref_deck = tight(c.deck);
            let raw = match run_ngspice(&ref_deck, c.id) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{:20} ngspice failed: {e}", c.id);
                    continue;
                }
            };
            let axis = if raw.plot.to_lowercase().contains("operating") {
                vec![0.0]
            } else {
                raw.cols[0].clone()
            };
            for p in c.probes {
                let ng = match raw.col(p) {
                    Some(v) => v.clone(),
                    None => {
                        eprintln!("{:20} {p}: not in ngspice output", c.id);
                        continue;
                    }
                };
                let t0 = std::time::Instant::now();
                match evaluate(c.deck, c.spec, p, &axis) {
                    Ok(o) => {
                        let swing = ng.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                            - ng.iter().cloned().fold(f64::INFINITY, f64::min);
                        let amax = ng.iter().fold(0.0f64, |m, v| m.max(v.abs()));
                        let denom = if c.spec == "tran_lti" {
                            swing.max(1e-12 * amax).max(1e-300)
                        } else {
                            amax.max(1e-300)
                        };
                        let err = o
                            .iter()
                            .zip(&ng)
                            .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()))
                            / denom;
                        eprintln!(
                            "{:20} {:8} pts={:4} norm_err={err:.3e} ({:.2}s)",
                            c.id,
                            p,
                            axis.len(),
                            t0.elapsed().as_secs_f64()
                        );
                        let lim = if c.spec == "tran_lti" { 5e-5 } else { 1e-8 };
                        assert!(err <= lim, "{} {p}: {err:e} > {lim:e}", c.id);
                        worst = worst.max(err);
                    }
                    Err(e) => panic!("{} {p}: oracle error {e}", c.id),
                }
            }
        }
        eprintln!("worst normalised error {worst:e}");
    }

    /// Rule 8 calibration of every static corpus deck that declares `bench.oracle:`: the oracle is evaluated on
    /// ngspice-47's own output axis (tight options, black box) and compared per probe. Prints one line per deck with the
    /// worst error as a fraction of the deck's tolerance (`FLAG` above 0.1, `unsupported` when the oracle refuses).
    /// `H2_WORK=/dev/shm/x F1_CASES=<dir> cargo test --release -p rustspice-cli static_corpus_oracle_vs_ngspice_tight -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn static_corpus_oracle_vs_ngspice_tight() {
        use super::testutil::*;
        if !ngspice_ok() {
            eprintln!("ngspice not available: skipped");
            return;
        }
        fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(d) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, out);
                    } else if p.extension().is_some_and(|x| x == "sp") {
                        out.push(p);
                    }
                }
            }
        }
        fn meta(deck: &str, key: &str) -> Option<String> {
            let pat = format!("* bench.{key}:");
            deck.lines()
                .find_map(|l| l.strip_prefix(&pat).map(|v| v.trim().to_string()))
        }
        let root = std::env::var("F1_CASES").unwrap_or_else(|_| "benchmarks/cases".to_string());
        let mut files = Vec::new();
        walk(std::path::Path::new(&root), &mut files);
        files.sort();
        let (mut n_ok, mut n_flag, mut n_unsup, mut worst_ratio) = (0, 0, 0, 0.0f64);
        for f in files {
            let Ok(deck) = std::fs::read_to_string(&f) else {
                continue;
            };
            let Some(spec) = meta(&deck, "oracle") else {
                continue;
            };
            let id = f.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
            let tran = deck
                .lines()
                .any(|l| l.to_ascii_lowercase().starts_with(".tran"));
            let tol_rel = meta(&deck, "tol.rel")
                .and_then(|v| parse_number(&v))
                .unwrap_or(1e-6);
            let tol_abs = meta(&deck, "tol.abs")
                .and_then(|v| parse_number(&v))
                .unwrap_or(1e-9);
            let tol_norm = meta(&deck, "tol.norm")
                .and_then(|v| parse_number(&v))
                .unwrap_or(1e-2);
            let floor = meta(&deck, "floor")
                .and_then(|v| parse_number(&v))
                .unwrap_or(1e-6);
            let raw = match run_ngspice(&tight(&deck), &format!("sc_{id}")) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{id:44} {spec:24} ngspice-tight failed: {e}");
                    continue;
                }
            };
            let axis = if raw.plot.to_lowercase().contains("operating") {
                vec![0.0]
            } else {
                raw.cols[0].clone()
            };
            let probes: Vec<String> = match meta(&deck, "probes") {
                Some(p) => p
                    .split(',')
                    .map(|x| x.trim().to_ascii_lowercase())
                    .collect(),
                None => raw
                    .names
                    .iter()
                    .filter(|n| n.starts_with("v("))
                    .take(8)
                    .cloned()
                    .collect(),
            };
            let (mut ratio, mut abs_worst, mut shown_err) = (0.0f64, 0.0f64, None);
            let mut used = 0;
            for p in &probes {
                let Some(ng) = raw.col(p) else { continue };
                match evaluate(&deck, &spec, p, &axis) {
                    Ok(o) => {
                        used += 1;
                        if tran {
                            let hi = ng.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                            let lo = ng.iter().cloned().fold(f64::INFINITY, f64::min);
                            let norm = o
                                .iter()
                                .zip(ng)
                                .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()))
                                / (hi - lo).max(floor);
                            abs_worst = abs_worst.max(norm);
                            ratio = ratio.max(norm / tol_norm);
                        } else {
                            for (a, b) in o.iter().zip(ng) {
                                let e = (a - b).abs();
                                abs_worst = abs_worst.max(e);
                                ratio = ratio.max(e / (tol_abs + tol_rel * b.abs()));
                            }
                        }
                    }
                    Err(e) => shown_err = Some(e),
                }
            }
            if used == 0 {
                n_unsup += 1;
                eprintln!(
                    "{id:44} {spec:24} unsupported: {}",
                    shown_err.unwrap_or_else(|| "no probe found in the ngspice output".into())
                );
                continue;
            }
            worst_ratio = worst_ratio.max(ratio);
            let flag = if ratio > 0.1 {
                n_flag += 1;
                "FLAG"
            } else {
                n_ok += 1;
                "ok"
            };
            eprintln!(
                "{id:44} {spec:24} pts={:5} worst={abs_worst:.2e} ({ratio:.2e} of tol) {flag}",
                axis.len()
            );
        }
        eprintln!("ok {n_ok}, flagged {n_flag}, unsupported {n_unsup}, worst fraction of tolerance {worst_ratio:.3e}");
    }

    /// Evaluates one probe of a deck file on a uniform axis and prints timing and end values (by hand):
    /// `F1_DECK=<file> F1_SPEC=tran_lti F1_PROBE='v(out)' F1_AXIS=<t0>:<t1>:<n> cargo test --release ... eval_deck_file -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn eval_deck_file() {
        let deck = std::fs::read_to_string(std::env::var("F1_DECK").expect("F1_DECK")).unwrap();
        let spec = std::env::var("F1_SPEC").unwrap_or_else(|_| "tran_lti".into());
        let probe = std::env::var("F1_PROBE").unwrap_or_else(|_| "v(out)".into());
        let ax = std::env::var("F1_AXIS").unwrap_or_else(|_| "0:1e-6:101".into());
        let p: Vec<f64> = ax.split(':').map(|v| v.parse().unwrap()).collect();
        let n = p[2] as usize;
        let axis: Vec<f64> = (0..n)
            .map(|k| p[0] + (p[1] - p[0]) * k as f64 / (n - 1) as f64)
            .collect();
        let t0 = std::time::Instant::now();
        let r = evaluate(&deck, &spec, &probe, &axis);
        match r {
            Ok(v) => eprintln!(
                "ok in {:.2}s: first {:e} last {:e}",
                t0.elapsed().as_secs_f64(),
                v[0],
                v[n - 1]
            ),
            Err(e) => eprintln!("ERR in {:.2}s: {e}", t0.elapsed().as_secs_f64()),
        }
    }

    // ---- E3a hardening: repro tests (they fail on the pre-hardening oracle) ----------------------------------

    /// ER-metrics-10: ngspice-47 reads `1a` as 1e-18 (checked with the black-box executable); the oracle read 1.
    #[test]
    fn atto_suffix_is_not_read_as_one() {
        for (s, v) in [
            ("1a", 1e-18),
            ("5A", 5e-18),
            ("3amp", 3e-18),
            ("2aF", 2e-18),
            ("1e3a", 1e-15),
        ] {
            let got = parse_number(s).unwrap_or_else(|| panic!("no parse {s}"));
            assert!((got - v).abs() <= 1e-12 * v, "{s}: {got} vs {v}");
        }
        // letters that are not scale factors stay units
        assert_eq!(parse_number("7V"), Some(7.0));
        assert_eq!(parse_number("4x"), Some(4.0));
    }

    fn chain_deck(rs: &[f64]) -> String {
        let mut d = String::from("chain\nV1 n0 0 1\n");
        for (k, r) in rs.iter().enumerate() {
            d.push_str(&format!("R{} n{} n{} {r:e}\n", k + 1, k, k + 1));
        }
        d.push_str(&format!("Rl n{} 0 1e3\n.op\n.end\n", rs.len()));
        d
    }

    /// ER-metrics-07: the module doc says ill-conditioned systems are rejected. A positive-feedback VCCS that cancels
    /// the resistor to 1e-13 of its value passes the backward-error test (a tiny residual) but the stamped entry
    /// `1 - 0.9999999999999` carries a cancellation error of 1e-3 of itself: the solution (1e13 V) is good to three
    /// digits at best. A merely badly SCALED system is not ill-conditioned and stays accepted.
    #[test]
    fn ill_conditioned_systems_are_rejected_and_badly_scaled_ones_still_solve() {
        let fb = "feedback\nI1 0 n1 1\nR1 n1 0 1\nG1 n1 0 n1 0 -0.9999999999999\n.op\n.end\n";
        let r = evaluate(fb, "dc_linear", "v(n1)", &[0.0]);
        assert!(
            r.as_ref().is_err_and(|e| e.contains("ill-conditioned")),
            "accepted: {r:?}"
        );
        let b = take_forward_error_bound().unwrap();
        assert!(b > 1e-5, "bound {b:e}");
        // the same loop with a gain that leaves 1e-3 of the resistor is a well-posed problem
        let ok = "feedback\nI1 0 n1 1\nR1 n1 0 1\nG1 n1 0 n1 0 -0.999\n.op\n.end\n";
        let v = evaluate(ok, "dc_linear", "v(n1)", &[0.0]).unwrap()[0];
        assert!((v / 1000.0 - 1.0).abs() < 1e-9, "{v}");
        // a 1 nohm / 1 nohm divider has cond_1 = 1e18 and is solved to full precision
        let div = "div\nV1 1 0 1\nR1 1 2 1n\nR2 2 0 1n\n.op\n.end\n";
        let v = evaluate(div, "dc_linear", "v(2)", &[0.0]).unwrap()[0];
        assert!((v - 0.5).abs() < 1e-15, "{v}");
        let b = take_forward_error_bound().unwrap();
        assert!(b < 1e-14, "bound {b:e}");
        // moderately conditioned chain: still solved
        let mild = chain_deck(&[1.0, 10.0, 100.0, 1e3, 1e4, 1e5]);
        let v = evaluate(&mild, "dc_linear", "v(n3)", &[0.0]).unwrap()[0];
        assert!(v.is_finite() && v > 0.0 && v < 1.0);
    }

    fn diode_string(n: usize) -> String {
        let mut d = String::from("diode string\nV1 n0 0 1000\nR1 n0 d1 1000\n");
        for k in 1..n {
            d.push_str(&format!("D{k} d{k} d{} dm\n", k + 1));
        }
        d.push_str(&format!(
            "D{n} d{n} 0 dm\n.model dm D(is=1e-14 n=1)\n.op\n.end\n"
        ));
        d
    }

    /// ER-metrics-08: a 300-diode string at 1 kV used to run 20-70 s per probe (and 4 probes per case) before Newton
    /// gave up; the wall-clock budget cuts it off and the other probes fail at once.
    #[test]
    fn wall_clock_budget_cuts_off_the_diode_string_and_the_later_probes_fail_at_once() {
        let d = diode_string(300);
        let budget = OracleBudget::new(Duration::from_millis(400));
        let t0 = Instant::now();
        let r = evaluate_with_budget(&d, "dc_nonlinear", "v(d150)", &[0.0], &budget);
        let first = t0.elapsed();
        let e = r.expect_err("the oracle cannot finish this deck in 400 ms");
        assert!(is_budget_error(&e), "{e}");
        assert!(first < Duration::from_secs(5), "cut off after {first:?}");
        // the other probes of the same deck do not repeat the budget
        let t1 = Instant::now();
        for p in ["v(d1)", "v(d300)", "i(v1)"] {
            let e = evaluate_with_budget(&d, "dc_nonlinear", p, &[0.0], &budget).unwrap_err();
            assert!(is_budget_error(&e), "{p}: {e}");
        }
        assert!(
            t1.elapsed() < Duration::from_millis(400),
            "{:?}",
            t1.elapsed()
        );
        // a budget error is not a statement about the deck: a generous budget on a small deck still solves
        let small = diode_string(3).replace("1000\nR1", "5\nR1");
        let ok = evaluate_with_budget(
            &small,
            "dc_nonlinear",
            "v(d2)",
            &[0.0],
            &OracleBudget::new(Duration::from_secs(20)),
        );
        assert!(ok.is_ok(), "{ok:?}");
    }

    /// ER-metrics-08: the pulse train (3 unknowns, 1 ns time constant, 11 ns period over 0.2 ms) used to spend ~45 s
    /// per probe before the operation count tripped.
    #[test]
    fn wall_clock_budget_cuts_off_a_small_transient_that_the_operation_count_misses() {
        let d = "pulse train\nV1 in 0 PULSE(0 1 0 1n 1n 4n 11n)\nR1 in out 1k\nC1 out 0 1p\n.tran 10n 0.2m\n.end\n";
        let axis: Vec<f64> = (0..=20_000).map(|k| k as f64 * 10e-9).collect();
        let t0 = Instant::now();
        let r = evaluate_with_budget(
            d,
            "tran_lti",
            "v(out)",
            &axis,
            &OracleBudget::new(Duration::from_secs(1)),
        );
        let e = r.expect_err("not solvable in 1 s");
        assert!(is_budget_error(&e), "{e}");
        assert!(t0.elapsed() < Duration::from_secs(8), "{:?}", t0.elapsed());
    }

    #[test]
    fn per_step_overhead_is_charged_for_small_systems() {
        let net = parse_deck(
            "rc\nV1 in 0 PULSE(0 1 0 1n 1n 4n 11n)\nR1 in out 1k\nC1 out 0 1p\n.tran 1n 1u\n.end\n",
        )
        .unwrap();
        let m = build_mna(&net, false, true, TRAN_UNKNOWN_LIMIT).unwrap();
        let ainv = inv3(&radau_a());
        let rowsum = [
            ainv[0].iter().sum::<f64>(),
            ainv[1].iter().sum::<f64>(),
            ainv[2].iter().sum::<f64>(),
        ];
        let n = m.n;
        let mut sys = TranSys {
            net: &net,
            m,
            ainv,
            rowsum,
            cache: Vec::new(),
            ops: 0.0,
        };
        let x = vec![0.0; n];
        sys.step(0.0, 1e-9, &x).unwrap(); // factors
        let before = sys.ops;
        sys.step(1e-9, 1e-9, &x).unwrap(); // reuses the factorisation
        assert!(
            sys.ops - before >= STEP_OVERHEAD_OPS,
            "{} ops for one step of {n} unknowns",
            sys.ops - before
        );
    }

    #[test]
    fn case_timeout_sets_the_default_budget() {
        assert_eq!(
            wall_budget_for_case_timeout(Duration::from_secs(60)),
            Duration::from_secs(10)
        );
        assert_eq!(
            wall_budget_for_case_timeout(Duration::from_secs(20)),
            Duration::from_secs(10)
        );
        assert_eq!(
            wall_budget_for_case_timeout(Duration::from_secs(8)),
            Duration::from_secs(4)
        );
        assert!(is_budget_error(
            "work budget exceeded before the step-halving test converged"
        ));
        assert!(is_budget_error(&check_deadline_for_test()));
        assert!(!is_budget_error("singular matrix"));
    }

    fn check_deadline_for_test() -> String {
        let _g = DeadlineGuard::set(Duration::ZERO);
        check_deadline().unwrap_err()
    }

    #[test]
    fn transpose_solve_and_condition_estimate_match_the_exact_values() {
        // A^T x = b against a direct residual, on a matrix that needs row interchanges
        let n = 6;
        let mut st = 0x1234_5678_9abc_def0u64;
        let mut rnd = move || {
            st ^= st << 13;
            st ^= st >> 7;
            st ^= st << 17;
            (st >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        let a: Vec<f64> = (0..n * n).map(|_| rnd()).collect();
        let mut lu = a.clone();
        let piv = lu_factor(n, &mut lu).unwrap();
        let b: Vec<f64> = (0..n).map(|_| rnd()).collect();
        let mut x = b.clone();
        lu_solve_t(n, &lu, &piv, &mut x);
        for j in 0..n {
            let r: f64 = (0..n).map(|i| a[i * n + j] * x[i]).sum::<f64>() - b[j];
            assert!(r.abs() < 1e-12, "col {j}: {r}");
        }
        // Hilbert 8x8: cond_1 = 3.387e10; the estimate is a lower bound within a factor of 3
        let h = 8;
        let hil: Vec<f64> = (0..h * h)
            .map(|k| 1.0 / ((k / h + k % h + 1) as f64))
            .collect();
        let mut lu = hil.clone();
        let piv = lu_factor(h, &mut lu).unwrap();
        let norm1 = (0..h)
            .map(|j| (0..h).map(|i| hil[i * h + j].abs()).sum::<f64>())
            .fold(0.0, f64::max);
        let cond = norm1 * inv_norm1_estimate(h, &lu, &piv);
        assert!(cond > 1e10 && cond < 4e10, "estimate {cond:e}");
        // diag(1, 1e-12): cond 1e12 exactly
        let mut d = vec![0.0; 4];
        d[0] = 1.0;
        d[3] = 1e-12;
        let r = solve_dense_refined_report(2, &d, &[1.0, 1.0]).unwrap();
        assert!((r.cond_est / 1e12 - 1.0).abs() < 1e-9, "{:e}", r.cond_est);
        // ... which is badly scaled, not ill-conditioned: the componentwise bound is a few u, not 1e12 u
        assert!(r.forward_bound < 1e-14, "{:e}", r.forward_bound);
        assert!(solve_dense_refined(2, &d, &[1.0, 1.0]).is_ok());
        // a really ill-conditioned matrix [[1,1],[1,1+1e-12]] x = [2, 2+1e-12] (x = [1,1]): bound ~ 1e12 * u
        let ill = [1.0, 1.0, 1.0, 1.0 + 1e-12];
        let r = solve_dense_refined_report(2, &ill, &[2.0, 2.0 + 1e-12]).unwrap();
        assert!(
            r.forward_bound > 1e-6 && r.forward_bound < 1e-2,
            "{:e}",
            r.forward_bound
        );
        assert!(r.cond_est > 1e11, "{:e}", r.cond_est);
        assert!(solve_dense_refined(2, &ill, &[2.0, 2.0 + 1e-12])
            .unwrap_err()
            .contains("forward-error bound"));
        let mild = [2.0, 1.0, 1.0, 3.0];
        assert!(solve_dense_refined(2, &mild, &[1.0, 2.0]).is_ok());
    }

    #[test]
    fn forward_error_bound_is_reported_for_linear_decks_only() {
        let mild = chain_deck(&[1.0, 10.0, 100.0, 1e3, 1e4, 1e5]);
        let _ = take_forward_error_bound();
        evaluate(&mild, "dc_linear", "v(n3)", &[0.0]).unwrap();
        let b = take_forward_error_bound().expect("a dense solve was made");
        assert!(b > 0.0 && b < FWD_BOUND_LIMIT, "{b:e}");
        assert!(take_forward_error_bound().is_none(), "read-and-clear");
        // a cache hit for the second probe reports the bound again
        evaluate(&mild, "dc_linear", "v(n3)", &[0.0]).unwrap();
        evaluate(&mild, "dc_linear", "v(n4)", &[0.0]).unwrap();
        assert!(take_forward_error_bound().is_some());
        // the closed-form ladder solves nothing
        evaluate("x\n", "ladder(n=10,rs=1,rsh=100)", "v(n3)", &[0.0]).unwrap();
        assert!(take_forward_error_bound().is_none());
    }

    /// A solution near the overflow limit (1e308 V across a 1k:3k divider, corpus deck `rob_hs_num_boundary_v_dblmax`)
    /// is a perfectly conditioned problem: the forward-error bound must not overflow into a rejection.
    #[test]
    fn forward_error_bound_does_not_overflow_for_huge_but_well_conditioned_solutions() {
        let d = "huge\nV1 1 0 1e308\nR1 1 2 1k\nR2 2 0 3k\n.op\n.end\n";
        let _ = take_forward_error_bound();
        let v =
            evaluate(d, "dc_linear", "v(2)", &[0.0]).expect("well-conditioned, finite solution");
        assert!((v[0] / 7.5e307 - 1.0).abs() < 1e-14, "{v:?}");
        let b = take_forward_error_bound().unwrap();
        assert!(b.is_finite() && b < 1e-14, "bound {b:e}");
        // and a non-finite solution still yields an infinite bound (rejected)
        let r = solve_dense_refined_report(1, &[1.0], &[f64::INFINITY]);
        assert!(r.is_err() || !r.unwrap().forward_bound.is_finite());
    }

    /// Exploration helper (`cargo test -- --ignored er_ill_dump --nocapture` with `ER_REQ=<json>`): the value and the
    /// forward-error bound of each request with the rejection limit lifted.
    #[test]
    #[ignore]
    fn er_ill_dump() {
        let Ok(path) = std::env::var("ER_REQ") else {
            eprintln!("ER_REQ not set");
            return;
        };
        let reqs: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        FWD_LIMIT.with(|c| c.set(f64::INFINITY));
        let mut out = Vec::new();
        for r in reqs.as_array().unwrap() {
            let axis: Vec<f64> = r["axis"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap())
                .collect();
            let v = evaluate(
                r["deck"].as_str().unwrap(),
                r["spec"].as_str().unwrap(),
                r["probe"].as_str().unwrap(),
                &axis,
            );
            let b = take_forward_error_bound();
            out.push(serde_json::json!({"value": v.as_ref().ok().map(|v| v[0]), "err": v.err(), "bound": b}));
        }
        if let Ok(p) = std::env::var("ER_OUT") {
            std::fs::write(p, serde_json::to_string(&out).unwrap()).unwrap();
        }
    }

    #[test]
    fn spec_budget_option_is_split_off_and_capped() {
        let (core, b) = split_budget_option("dc_nonlinear(gmin=1e-12,budget=30)").unwrap();
        assert_eq!(core, "dc_nonlinear(gmin=1e-12)");
        assert!(b.unwrap() <= Duration::from_secs(30) && b.unwrap() >= Duration::from_secs(1));
        let (core, b) = split_budget_option("tran_lti(budget=5)").unwrap();
        assert_eq!(
            (core.as_str(), b),
            ("tran_lti", Some(Duration::from_secs(5)))
        );
        assert_eq!(
            split_budget_option("dc_linear").unwrap(),
            ("dc_linear".to_string(), None)
        );
        assert_eq!(
            split_budget_option("dc_nonlinear(gmin=1e-9)").unwrap(),
            ("dc_nonlinear(gmin=1e-9)".to_string(), None)
        );
        assert!(split_budget_option("tran_lti(budget=abc)").is_err());
        assert!(split_budget_option("tran_lti(budget=-1)").is_err());
        // an absurd request is capped
        let (_, b) = split_budget_option("dc_linear(budget=1e9)").unwrap();
        assert!(b.unwrap() <= MAX_SPEC_BUDGET);
        // end to end: the spec option reaches the evaluation (a tiny budget cuts a deck the caller's budget would solve)
        let d = diode_string(300);
        let t0 = Instant::now();
        let e = evaluate_with_budget(
            &d,
            "dc_nonlinear(budget=0.3)",
            "v(d150)",
            &[0.0],
            &OracleBudget::new(Duration::from_secs(60)),
        )
        .unwrap_err();
        assert!(
            is_budget_error(&e) && t0.elapsed() < Duration::from_secs(10),
            "{e} after {:?}",
            t0.elapsed()
        );
        // a deck without the option still solves with the caller's budget
        let ok = evaluate_with_budget(
            &chain_deck(&[1.0, 2.0]),
            "dc_linear(budget=5)",
            "v(n1)",
            &[0.0],
            &OracleBudget::default(),
        );
        assert!(ok.is_ok(), "{ok:?}");
    }
}
