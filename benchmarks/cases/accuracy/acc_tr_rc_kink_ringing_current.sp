Trapezoid ringing of i(v1) after the slope kinks of a slow PULSE ramp, tstep = 100 tau
* bench.id: acc_tr_rc_kink_ringing_current
* bench.group: accuracy
* bench.title: RC (tau = 1 ns) driven by a 1 us ramp (kinks on the grid) with tstep = 100 ns = 100 tau: i(v1) alternates in sign on the trapezoid rule
* bench.probes: i(v1), v(out)
* bench.tol.norm: 5e-2
* bench.ref.tmax: 0.5n
* bench.floor: 1e-7
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: linear, tran, coarse_step, ringing, branch_current
* bench.est_rss_mib: 16
* bench.note: ideal ramp current i(v1) = -C dv/dt = -1 uA on the rising edge, +1 uA on the falling edge; converged ngspice (tmax 0.5 ns so the 1 ns kink transients are resolved) and oracle tran_lti agree; ngspice as-is is smooth (2.2e-2 of swing off, which is the kink-instant interpolation of its LTE-chosen rows) and the tolerance 5e-2 sits above that level; flip test (same deck): tstep = 1 ns (= tau) passes at 2.1e-2, tstep = 10 ns still fails at 0.33, tstep = 100 ns fails at 0.55
V1 in 0 PULSE(0 1 1u 1u 1u 2u 10u)
R1 in out 1k
C1 out 0 1p
.tran 100n 10u
.end
