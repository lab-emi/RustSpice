Diode I-V sweep through 100 ohm, 4097 points
* bench.id: perf_dc_diode_iv_4k
* bench.group: performance
* bench.title: Diode I-V sweep through 100 ohm, 4097 points
* bench.probes: v(d), i(v1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 3
* bench.est_rss_mib: 10
* bench.tags: nonlinear, dc, sweep, diode, jit_tier
* bench.note: 4097 points over a diode knee from deep off to about 35 mA, the longest single sweep on the smallest nonlinear circuit; step 2^-10 V keeps the swept grid exact
.model dm D(IS=1e-14 N=1)
V1 in 0 0
R1 in d 100
D1 d 0 dm
.dc V1 0 4 0.0009765625
.end
