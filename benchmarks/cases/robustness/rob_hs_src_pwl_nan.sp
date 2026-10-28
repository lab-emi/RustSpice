PWL with a nan value
* bench.id: rob_hs_src_pwl_nan
* bench.group: robustness
* bench.title: PWL with a nan value
* bench.probes: v(2)
* bench.expect: loud_error
* bench.tags: hostile, source, nonfinite, tran, silent_risk
* bench.est_rss_mib: 64
* bench.note: Not nan handling as such: a valid PWL(0 0 1u 1 2u 0) is also DC 0 V in RustSpice (ngspice peak 1.0 V), so the non-finite value is never evaluated. It stays a gap until RustSpice refuses the non-finite value; parsing PWL alone does not close it [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 PWL(0 0 1u nan 2u 0)
R1 1 2 1k
C1 2 0 100p
.tran 10n 2u
.end
