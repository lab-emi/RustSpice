Series-driven LC tank, Q=63, near-resonant sine, 1e5 steps
* bench.id: perf_lin_lc_tank_sine_1e5
* bench.group: performance
* bench.title: Series-driven LC tank, Q=63, near-resonant sine, 1e5 steps
* bench.probes: v(b), i(v1)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 5n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 5
* bench.est_rss_mib: 25
* bench.tags: linear, tran, jit_tier, long, resonant
* bench.note: lightly damped resonant circuit driven near f0 for 50 periods; amplitude grows through the run so a wrong frequency or damping shows up in the waveform
V1 in 0 SIN(0 1 100k)
R1 in a 10
L1 a b 1m
C1 b 0 2.53n
.tran 5n 500u
.end
