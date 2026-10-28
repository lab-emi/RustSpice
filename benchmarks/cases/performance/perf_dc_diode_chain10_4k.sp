Ten-diode series chain through 100 ohm, 4097 sweep points
* bench.id: perf_dc_diode_chain10_4k
* bench.group: performance
* bench.title: Ten-diode series chain through 100 ohm, 4097 sweep points
* bench.probes: v(d5), i(v1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 12
* bench.est_rss_mib: 15
* bench.tags: nonlinear, dc, sweep, diode, jit_tier
* bench.note: 12 unknowns; the chain stays off until about 6.5 V and then conducts, so Newton iteration counts change mid-sweep; step 2^-9 V
.model dm D(IS=1e-14 N=1)
V1 in 0 0
R1 in d0 100
D1 d0 d1 dm
D2 d1 d2 dm
D3 d2 d3 dm
D4 d3 d4 dm
D5 d4 d5 dm
D6 d5 d6 dm
D7 d6 d7 dm
D8 d7 d8 dm
D9 d8 d9 dm
D10 d9 0 dm
.dc V1 0 8 0.001953125
.end
