Half-wave diode rectifier with RC load, 1 kHz, 5e4 steps
* bench.id: perf_nl_diode_rect_rc_tran
* bench.group: performance
* bench.title: Half-wave diode rectifier with RC load, 1 kHz, 5e4 steps
* bench.probes: v(out), i(v1)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 100n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 3
* bench.est_rss_mib: 20
* bench.tags: nonlinear, tran, diode, rectifier, jit_tier
* bench.note: half-wave rectifier: the diode switches every half cycle, so every step is a Newton solve of a 3-unknown system
.model dm D(IS=1e-14 N=1)
V1 in 0 SIN(0 5 1k)
D1 in out dm
R1 out 0 1k
C1 out 0 10u
.tran 100n 5m
.end
