Fifth-order Butterworth LC low-pass (50 ohm, 1 MHz), pulse drive, 1e5 steps
* bench.id: perf_lin_butter5_step_1e5
* bench.group: performance
* bench.title: Fifth-order Butterworth LC low-pass (50 ohm, 1 MHz), pulse drive, 1e5 steps
* bench.probes: v(out), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 100p
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 7
* bench.est_rss_mib: 25
* bench.tags: linear, tran, jit_tier, long, filter
* bench.note: doubly terminated ladder filter, step response with ringing; 7 unknowns (4 nodes plus the V and two inductor branches)
V1 s 0 PULSE(0 1 0 20n 20n 5u 10u)
Rs s in 50
C1 in 0 1.967n
L2 in b 12.88u
C3 b 0 6.366n
L4 b out 12.88u
C5 out 0 1.967n
RL out 0 50
.tran 100p 10u
.end
