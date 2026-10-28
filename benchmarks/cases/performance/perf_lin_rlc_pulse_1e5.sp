Series RLC, Q=5, periodic pulse drive, 1e5 steps
* bench.id: perf_lin_rlc_pulse_1e5
* bench.group: performance
* bench.title: Series RLC, Q=5, periodic pulse drive, 1e5 steps
* bench.probes: v(b), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 40n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 5
* bench.est_rss_mib: 25
* bench.tags: linear, tran, jit_tier, long, underdamped
* bench.note: underdamped series RLC (f0 about 5 kHz), two drive periods; the inductor branch makes the system 5 unknowns
V1 in 0 PULSE(0 1 0 2u 2u 1m 2m)
R1 in a 6.3
L1 a b 1m
C1 b 0 1u
.tran 40n 4m
.end
