//! Descriptive statistics, log-log least-squares fits and work-precision interpolation.
//!
//! Every function returns `None` / a status string instead of a number when the input cannot support the
//! quantity (Rule 4: never report an unmeasured value as zero).

use serde::Serialize;

/// Summary of repeated measurements (all in the unit of the samples).
///
/// Fewer than [`MIN_SPREAD_SAMPLES`] finite samples cannot show that a measurement is stable: such a summary is
/// flagged `noisy = true` (a consumer gating on `!noisy` never accepts it as measured) and carries
/// `spread_status != "ok"`; `stddev` is `NaN` (JSON `null`) for a single sample, because an unmeasurable spread is not
/// zero (Rule 4).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Summary {
    pub n: usize,
    pub min: f64,
    pub median: f64,
    pub mean: f64,
    /// Sample standard deviation; `NaN` (serialised as `null`) when `n < 2`.
    pub stddev: f64,
    /// `true` when `stddev / median > 0.2` or when `n < 3` (spread not measured, see `spread_status`).
    pub noisy: bool,
    pub samples: Vec<f64>,
    /// `ok` (n >= 3) | `two_samples` | `single_sample`: whether `stddev` / `noisy` rest on enough samples.
    pub spread_status: &'static str,
    /// Non-finite input samples that were dropped (`n` counts the finite ones only).
    pub dropped_non_finite: usize,
}

/// Relative spread above which a measurement series is flagged noisy.
pub const NOISY_THRESHOLD: f64 = 0.2;

/// Finite samples needed before a spread estimate (and hence `noisy = false`) means anything.
pub const MIN_SPREAD_SAMPLES: usize = 3;

/// Summarise finite samples. Returns `None` for an empty (or all-non-finite) input.
pub fn summarize(samples: &[f64]) -> Option<Summary> {
    let mut v: Vec<f64> = samples.iter().copied().filter(|x| x.is_finite()).collect();
    let dropped_non_finite = samples.len() - v.len();
    if v.is_empty() {
        return None;
    }
    let raw = v.clone();
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    let median = if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    };
    let mean = v.iter().sum::<f64>() / n as f64;
    // Sample standard deviation; a single sample has no spread estimate (NaN, never 0).
    let stddev = if n > 1 {
        (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n as f64 - 1.0)).sqrt()
    } else {
        f64::NAN
    };
    let (spread_status, enough) = match n {
        1 => ("single_sample", false),
        2 => ("two_samples", false),
        _ => ("ok", true),
    };
    debug_assert_eq!(enough, n >= MIN_SPREAD_SAMPLES);
    let noisy = !enough || (median > 0.0 && stddev / median > NOISY_THRESHOLD);
    Some(Summary {
        n,
        min: v[0],
        median,
        mean,
        stddev,
        noisy,
        samples: raw,
        spread_status,
        dropped_non_finite,
    })
}

/// Result of a log-log least-squares fit `y = c * x^p`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fit {
    pub exponent: Option<f64>,
    pub prefactor: Option<f64>,
    pub r2: Option<f64>,
    pub points: usize,
    /// `ok` | `too_few_points` | `degenerate_x` | `non_positive_data`
    pub status: String,
}

/// Minimum number of points for a fit to be reported.
pub const MIN_FIT_POINTS: usize = 3;

/// Least-squares fit of `ln y = ln c + p ln x`. Points with non-positive or non-finite coordinates are dropped.
pub fn loglog_fit(points: &[(f64, f64)]) -> Fit {
    let pts: Vec<(f64, f64)> = points
        .iter()
        .copied()
        .filter(|(x, y)| x.is_finite() && y.is_finite() && *x > 0.0 && *y > 0.0)
        .map(|(x, y)| (x.ln(), y.ln()))
        .collect();
    let fail = |status: &str, n: usize| Fit {
        exponent: None,
        prefactor: None,
        r2: None,
        points: n,
        status: status.to_string(),
    };
    if pts.len() < MIN_FIT_POINTS {
        let s = if points.len() >= MIN_FIT_POINTS {
            "non_positive_data"
        } else {
            "too_few_points"
        };
        return fail(s, pts.len());
    }
    let n = pts.len() as f64;
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let syy: f64 = pts.iter().map(|p| (p.1 - my) * (p.1 - my)).sum();
    if sxx <= 1e-300 {
        return fail("degenerate_x", pts.len());
    }
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    let r2 = if syy <= 1e-300 {
        // y constant: the fit explains everything (exponent 0).
        1.0
    } else {
        (sxy * sxy) / (sxx * syy)
    };
    Fit {
        exponent: Some(slope),
        prefactor: Some(intercept.exp()),
        r2: Some(r2),
        points: pts.len(),
        status: "ok".to_string(),
    }
}

/// One level of a work-precision comparison.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EqualErrorLevel {
    pub err: f64,
    pub t_a: f64,
    pub t_b: f64,
    /// `t_b / t_a` (>1 means curve A is faster at this error).
    pub ratio_b_over_a: f64,
}

/// Interpolated "time at equal error" table.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EqualError {
    pub levels: Vec<EqualErrorLevel>,
    pub range: Option<(f64, f64)>,
    /// `ok` | `no_overlap` | `too_few_points`
    pub status: String,
    pub a_range: Option<(f64, f64)>,
    pub b_range: Option<(f64, f64)>,
    /// Points of curve A / B that did not enter the interpolation: non-finite, zero or negative error or time, and
    /// points dominated by a faster point of equal or better error (Pareto frontier). Never silently shrinks a range.
    pub a_dropped: usize,
    pub b_dropped: usize,
    /// Of those, the points with an error that is exactly 0 (an error below what the run could measure: the low end of
    /// the interpolation range stops at the smallest positive error).
    pub a_zero_error: usize,
    pub b_zero_error: usize,
}

/// A curve reduced to what the interpolation may use.
struct Frontier {
    /// Pareto frontier sorted by increasing error, strictly decreasing time.
    pts: Vec<(f64, f64)>,
    dropped: usize,
    zero_error: usize,
}

/// Pareto frontier of `(error, time)` points. For every error level the time of a curve is the minimum over all its
/// points with an error at or below that level, so a point that is slower than another one with a smaller (or equal)
/// error is never the answer to "how long does it take to reach this error" and is dropped. Non-finite points and points
/// with a non-positive error or time cannot be placed on a log-log curve and are counted, not hidden.
fn frontier(curve: &[(f64, f64)]) -> Frontier {
    let zero_error = curve
        .iter()
        .filter(|(e, t)| *e == 0.0 && t.is_finite() && *t > 0.0)
        .count();
    let mut v: Vec<(f64, f64)> = curve
        .iter()
        .copied()
        .filter(|(e, t)| e.is_finite() && t.is_finite() && *e > 0.0 && *t > 0.0)
        .collect();
    // by error, equal errors fastest first: the first point of a run of equal errors is the one that survives
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(v.len());
    for p in v {
        if pts.last().is_none_or(|l| p.1 < l.1) {
            pts.push(p);
        }
    }
    let kept = pts.len();
    Frontier {
        dropped: curve.len() - kept,
        pts,
        zero_error,
    }
}

/// Log-log piecewise-linear interpolation of `t` at error `e` on a Pareto frontier (increasing error).
fn interp_loglog(curve: &[(f64, f64)], e: f64) -> Option<f64> {
    if curve.len() < 2 || e < curve[0].0 || e > curve[curve.len() - 1].0 {
        return None;
    }
    for w in curve.windows(2) {
        let (e0, t0) = w[0];
        let (e1, t1) = w[1];
        if e >= e0 && e <= e1 {
            if (e1 / e0).ln() < 1e-12 {
                return Some(t0.min(t1));
            }
            let f = (e / e0).ln() / (e1 / e0).ln();
            return Some((t0.ln() + f * (t1.ln() - t0.ln())).exp());
        }
    }
    None
}

/// Time at equal error for two curves of `(error, time)` points (e.g. RustSpice and ngspice as the
/// tstep series is refined). Each curve is first reduced to its Pareto frontier (see [`frontier`]); levels are `levels`
/// log-spaced errors in the intersection of both error ranges.
pub fn time_at_equal_error(a: &[(f64, f64)], b: &[(f64, f64)], levels: usize) -> EqualError {
    let (fa, fb) = (frontier(a), frontier(b));
    let range_of = |c: &[(f64, f64)]| {
        if c.is_empty() {
            None
        } else {
            Some((c[0].0, c[c.len() - 1].0))
        }
    };
    let (ar, br) = (range_of(&fa.pts), range_of(&fb.pts));
    let (a, b) = (&fa.pts, &fb.pts);
    let mk = |levels: Vec<EqualErrorLevel>, range: Option<(f64, f64)>, status: &str| EqualError {
        levels,
        range,
        status: status.to_string(),
        a_range: ar,
        b_range: br,
        a_dropped: fa.dropped,
        b_dropped: fb.dropped,
        a_zero_error: fa.zero_error,
        b_zero_error: fb.zero_error,
    };
    if a.len() < 2 || b.len() < 2 {
        return mk(Vec::new(), None, "too_few_points");
    }
    let lo = a[0].0.max(b[0].0);
    let hi = a[a.len() - 1].0.min(b[b.len() - 1].0);
    if lo >= hi {
        return mk(Vec::new(), None, "no_overlap");
    }
    let k = levels.max(2);
    let mut out = Vec::new();
    for i in 0..k {
        let f = i as f64 / (k - 1) as f64;
        let e = (lo.ln() + f * (hi.ln() - lo.ln())).exp().clamp(lo, hi);
        if let (Some(ta), Some(tb)) = (interp_loglog(a, e), interp_loglog(b, e)) {
            out.push(EqualErrorLevel {
                err: e,
                t_a: ta,
                t_b: tb,
                ratio_b_over_a: tb / ta,
            });
        }
    }
    mk(out, Some((lo, hi)), "ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_odd_even_and_noisy() {
        let s = summarize(&[3.0, 1.0, 2.0]).unwrap();
        assert_eq!((s.n, s.min, s.median, s.mean), (3, 1.0, 2.0, 2.0));
        assert!((s.stddev - 1.0).abs() < 1e-12);
        assert!(s.noisy); // 1/2 > 0.2
        let s = summarize(&[10.0, 10.5, 9.5, 10.0]).unwrap();
        assert_eq!(s.median, 10.0);
        assert!(!s.noisy);
        assert_eq!(s.samples, vec![10.0, 10.5, 9.5, 10.0]);
        // fewer than three samples: the spread is not measured (see `short_series_are_not_reported_as_stable`)
        let s = summarize(&[4.0]).unwrap();
        assert!(s.noisy && s.stddev.is_nan() && s.n == 1);
        assert!(summarize(&[]).is_none());
        assert!(summarize(&[f64::NAN]).is_none());
    }

    #[test]
    fn fit_recovers_known_exponent() {
        // y = 3 x^1.5 exactly
        let pts: Vec<(f64, f64)> = [10.0, 100.0, 1000.0, 10000.0]
            .iter()
            .map(|&x| (x, 3.0 * f64::powf(x, 1.5)))
            .collect();
        let f = loglog_fit(&pts);
        assert_eq!(f.status, "ok");
        assert!((f.exponent.unwrap() - 1.5).abs() < 1e-12);
        assert!((f.prefactor.unwrap() - 3.0).abs() < 1e-9);
        assert!((f.r2.unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn fit_guards() {
        assert_eq!(
            loglog_fit(&[(1.0, 1.0), (2.0, 2.0)]).status,
            "too_few_points"
        );
        assert!(loglog_fit(&[(1.0, 1.0), (2.0, 2.0)]).exponent.is_none());
        assert_eq!(
            loglog_fit(&[(1.0, 1.0), (1.0, 2.0), (1.0, 3.0)]).status,
            "degenerate_x"
        );
        assert_eq!(
            loglog_fit(&[(1.0, 0.0), (2.0, 0.0), (3.0, 1.0)]).status,
            "non_positive_data"
        );
    }

    #[test]
    fn equal_error_interpolation() {
        // a: t = 1/err, b: t = 4/err  => ratio 4 everywhere in the overlap.
        let a: Vec<(f64, f64)> = [1e-4, 1e-3, 1e-2].iter().map(|&e| (e, 1.0 / e)).collect();
        let b: Vec<(f64, f64)> = [1e-5, 1e-3, 1e-2, 1e-1]
            .iter()
            .map(|&e| (e, 4.0 / e))
            .collect();
        let r = time_at_equal_error(&a, &b, 5);
        assert_eq!(r.status, "ok");
        let (lo, hi) = r.range.unwrap();
        assert!((lo - 1e-4).abs() < 1e-18 && (hi - 1e-2).abs() < 1e-18);
        assert_eq!(r.levels.len(), 5);
        for l in &r.levels {
            assert!((l.ratio_b_over_a - 4.0).abs() < 1e-6, "{l:?}");
        }
        // disjoint error ranges
        let c = vec![(1e-9, 1.0), (1e-8, 0.5)];
        let r = time_at_equal_error(&a, &c, 4);
        assert_eq!(r.status, "no_overlap");
        assert!(r.levels.is_empty());
        assert_eq!(time_at_equal_error(&a[..1], &b, 4).status, "too_few_points");
        // nothing was dropped from clean monotone curves
        let r = time_at_equal_error(&a, &b, 5);
        assert_eq!(
            (r.a_dropped, r.b_dropped, r.a_zero_error, r.b_zero_error),
            (0, 0, 0, 0)
        );
    }

    // ---- E3a hardening: every test below fails on the pre-hardening statistics ---------------------------

    /// ER-metrics-11: too few samples are never reported as a measured, stable series.
    #[test]
    fn short_series_are_not_reported_as_stable() {
        let one = summarize(&[5.0]).unwrap();
        assert_eq!(one.n, 1);
        assert!(one.noisy, "one sample cannot show stability");
        assert!(one.stddev.is_nan(), "an unmeasurable spread is not 0");
        assert_eq!(
            serde_json::to_value(&one).unwrap()["stddev"],
            serde_json::Value::Null
        );
        let two = summarize(&[5.0, 5.0]).unwrap();
        assert!(two.noisy && two.stddev == 0.0, "two samples: {two:?}");
        let three = summarize(&[5.0, 5.0, 5.0]).unwrap();
        assert!(!three.noisy && three.stddev == 0.0);
        // dropped non-finite samples can shrink n below the threshold: visible in the summary
        let d = summarize(&[5.0, f64::NAN, f64::INFINITY]).unwrap();
        assert_eq!(d.n, 1);
        assert!(d.noisy);
        assert_eq!(d.dropped_non_finite, 2);
        assert_eq!(summarize(&[1.0, 2.0, 3.0]).unwrap().dropped_non_finite, 0);
        assert_eq!(d.spread_status, "single_sample");
        assert_eq!(two.spread_status, "two_samples");
        assert_eq!(three.spread_status, "ok");
    }

    /// ER-metrics-06: a curve is reduced to its Pareto frontier before interpolation.
    #[test]
    fn equal_error_uses_the_pareto_frontier() {
        let a = [(1e-2, 10.0), (1e-3, 40.0), (1e-4, 500.0), (0.9e-4, 300.0)];
        let b = [(1e-2, 100.0), (1e-3, 200.0), (1e-4, 400.0)];
        let r = time_at_equal_error(&a, &b, 3);
        assert_eq!(r.status, "ok");
        let first = &r.levels[0];
        assert!((first.err - 1e-4).abs() < 1e-12);
        // RustSpice reached 0.9e-4 in 300 us: at an error of 1e-4 it needs at most 300 us
        assert!(first.t_a <= 300.0 + 1e-9, "t_a {}", first.t_a);
        assert!(
            first.ratio_b_over_a >= 1.33,
            "ratio {}",
            first.ratio_b_over_a
        );
        // time is non-increasing in the error level on a frontier
        for w in r.levels.windows(2) {
            assert!(w[1].t_a <= w[0].t_a * (1.0 + 1e-12), "{:?}", r.levels);
        }
        // duplicated errors keep the faster point on both sides
        let dup = [(1e-3, 50.0), (1e-3, 30.0), (1e-2, 10.0)];
        let r = time_at_equal_error(&dup, &b, 2);
        assert!((r.levels[0].t_a - 30.0).abs() < 1e-9, "{:?}", r.levels);
    }

    #[test]
    fn dropped_points_are_reported_not_hidden() {
        // a zero-error point, a non-finite one and a dominated one on curve A
        let a = [
            (0.0, 90.0),
            (1e-3, f64::NAN),
            (1e-3, 40.0),
            (2e-3, 60.0), // dominated by (1e-3, 40)
            (1e-2, 10.0),
        ];
        let b = [(1e-2, 100.0), (1e-3, 200.0)];
        let r = time_at_equal_error(&a, &b, 3);
        assert_eq!(r.status, "ok");
        assert_eq!((r.a_dropped, r.a_zero_error, r.b_dropped), (3, 1, 0));
        let (lo, _) = r.a_range.unwrap();
        assert_eq!(lo, 1e-3, "the zero-error point does not extend the range");
    }
}
