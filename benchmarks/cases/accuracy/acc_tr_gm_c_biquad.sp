gm-C band-pass biquad
* bench.id: acc_tr_gm_c_biquad
* bench.group: accuracy
* bench.title: Two-integrator gm-C loop (VCCS elements, f0 = 1.59 MHz, Q = 5) driven by a step: transient with controlled sources
* bench.probes: v(bp), v(lp)
* bench.tol.norm: 7e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, vccs, filter
* bench.est_rss_mib: 16
* bench.note: Calibration: RustSpice's error does not converge with the step (3.3e-3, 3.1e-3, 3.3e-3 of swing at tstep, /2, /4; ngspice 2.0e-3, 5e-4, 1.3e-4): the two-integrator loop stores the BDF-1 first-step offset (gm/C * tr/2 = 1e7 V/s * 0.5 ns = 5e-3 V) of the 1 ns ramp, so this pass is bounded by tol 7e-3 and not by the step (same mechanism as acc_tr_rc_first_step_bdf1, tagged fixed-step-accuracy).
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
Gin 0 bp in 0 1m
C1 bp 0 100p
Gint 0 lp bp 0 1m
C2 lp 0 100p
Gfb bp 0 lp 0 1m
Gq bp 0 bp 0 0.2m
.tran 10n 4u
.end
