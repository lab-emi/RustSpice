transient with a 1e300 V pulse into 1 mohm + 1 F
* bench.id: rob_hs_num_tran_swing_1e300
* bench.group: robustness
* bench.title: transient with a 1e300 V pulse into 1 mohm + 1 F
* bench.probes: v(2)
* bench.expect: either
* bench.tags: hostile, numeric, overflow, tran
* bench.est_rss_mib: 64
* bench.note: ngspice aborts after two points ("Timestep too small; time = 1e-12, timestep = 2.5e-22"). RustSpice stays finite: v(2)(10 ns) = 9.99995e294 V versus the exact 9.49995e294 V (scipy ODE with the 1 ns ramp scaled by 1e300), i.e. 5.3 % high because the fixed 1 ns step sees the edge as a step (the fixed-step-accuracy effect of c_negative_growth); the shape is right and nothing overflows. ngspice-only failure, RustSpice not penalised
V1 1 0 PULSE(0 1e300 0 1n 1n 1 2)
R1 1 2 1e-3
C1 2 0 1
.tran 1n 10n
.end
