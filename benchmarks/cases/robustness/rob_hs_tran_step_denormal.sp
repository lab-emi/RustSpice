tran 1e-310 1e-305 (denormal step, 1e5 steps)
* bench.id: rob_hs_tran_step_denormal
* bench.group: robustness
* bench.title: .tran 1e-310 1e-305 (denormal step, 1e5 steps)
* bench.probes: v(2)
* bench.expect: either
* bench.tags: hostile, tran, args, scale, resource
* bench.est_rss_mib: 64
* bench.note: ngspice stops at the first step ("doAnalyses: TRAN: Timestep too small; initial timepoint", exit 1, no data); RustSpice integrates 1e5 steps of 1e-310 s in 0.03 s: v(1)(tstop) = 4.9999999999999994e-296 V = 5*tstop/1ns exactly and v(2) underflows to 0. A denormal step has no physical meaning, so neither refusing nor computing can be judged: ambiguous
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1n
.tran 1e-310 1e-305
.end
