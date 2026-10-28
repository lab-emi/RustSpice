RC driven by a PULSE whose whole ramp lies inside the first step
* bench.id: acc_tr_rc_first_step_bdf1
* bench.group: accuracy
* bench.title: RC (tau = 1 us) driven by PULSE(0 1 0 1n 1n ...) with tstep = tau/10: the 1 ns ramp lies inside the first step
* bench.probes: v(out)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: linear, tran, first_step, timing_edge
* bench.note: mechanism check: the closed recursion v1 = a/(1+a), v(k+1) = ((1-a/2) v(k) + a)/(1+a/2) with a = tstep/tau = 0.1 reproduces every RustSpice row to 1e-15, i.e. BDF-1 for step 1 then trapezoid; pure trapezoid from t = 0 would be within 3.1e-4 of the exact 1-exp(-t/tau); the oracle tran_lti and the converged ngspice agree with the exact curve to 1e-5
* bench.est_rss_mib: 16
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 100n 5u
.end
