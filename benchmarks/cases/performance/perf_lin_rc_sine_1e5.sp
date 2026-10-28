RC low-pass driven by a 5 kHz sine, 1e5 steps
* bench.id: perf_lin_rc_sine_1e5
* bench.group: performance
* bench.title: RC low-pass driven by a 5 kHz sine, 1e5 steps
* bench.probes: v(out), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 200n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 3
* bench.est_rss_mib: 20
* bench.tags: linear, tran, jit_tier, long
* bench.note: three-unknown linear circuit, 1e5 fixed steps: per-step cost of the cached-LU linear regime and rawfile output dominate
V1 in 0 SIN(0 1 5k)
R1 in out 1k
C1 out 0 100n
.tran 200n 20.0001m
.end
