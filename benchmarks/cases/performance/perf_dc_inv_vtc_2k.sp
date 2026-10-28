CMOS inverter transfer curve at VDD=2.5 V, 2561 sweep points
* bench.id: perf_dc_inv_vtc_2k
* bench.group: performance
* bench.title: CMOS inverter transfer curve at VDD=2.5 V, 2561 sweep points
* bench.probes: v(out), i(vdd)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 5
* bench.est_rss_mib: 10
* bench.tags: nonlinear, dc, sweep, cmos, jit_tier
* bench.note: 2561 warm-started Newton solves of a 5-unknown circuit through the high-gain region; per-point cost and any compile cost dominate; the step is 2^-10 V so the accumulated sweep grid is exact
.model nm NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.model pm PMOS(LEVEL=1 VTO=-0.7 KP=50u LAMBDA=0.02)
VDD vdd 0 2.5
VIN in 0 0
MP out in vdd vdd pm W=20u L=1u
MN out in 0 0 nm W=10u L=1u
.dc VIN 0 2.5 0.0009765625
.end
