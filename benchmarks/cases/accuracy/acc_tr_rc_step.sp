RC low-pass step response
* bench.id: acc_tr_rc_step
* bench.group: accuracy
* bench.title: RC low-pass step (tau = 1 us), 100 steps per tau
* bench.probes: v(out)
* bench.tol.norm: 2e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran
* bench.est_rss_mib: 16
* bench.note: Calibration: RustSpice's error does not shrink with the step (4.5e-4, 4.9e-4, 5.0e-4 at tstep, /2, /4) because the 1 ns PULSE ramp lies inside the BDF-1 first step: a floor of tr/(2 tau) = 5e-4 of swing, while ngspice converges (7e-6, 1e-6, 3e-7); tol 2e-3 sits above that floor (see acc_tr_rc_first_step_bdf1 for the coarse-step version, tagged fixed-step-accuracy).
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 10n 5u
.end
