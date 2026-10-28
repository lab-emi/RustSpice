RC low-pass step response, 100 time steps
* bench.id: perf_su_rc_step_100
* bench.group: performance
* bench.title: RC low-pass step response, 100 time steps
* bench.probes: v(out), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 3
* bench.est_rss_mib: 10
* bench.tags: startup, linear, tran
* bench.note: tiny linear transient of 100 steps; start-up, parse and the one-time factorisation dominate the wall time; the pulse starts after two steps and ramps over 20 steps so that the first backward-Euler step of a fixed-step integrator sees a constant source
V1 in 0 PULSE(0 1 40u 400u 400u 10 20)
R1 in out 1k
C1 out 0 1u
.tran 20u 2m
.end
