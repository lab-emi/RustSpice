NMOS common-source amplifier, 5 MHz sine on 1.2 V bias, 5e4 steps
* bench.id: perf_nl_cs_amp_sine_5e4
* bench.group: performance
* bench.title: NMOS common-source amplifier, 5 MHz sine on 1.2 V bias, 5e4 steps
* bench.probes: v(d), i(vdd)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 40p
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 5
* bench.est_rss_mib: 20
* bench.tags: nonlinear, tran, mos, amplifier, jit_tier
* bench.note: single-transistor amplifier driven through its non-linear region for ten periods; 5 unknowns
.model nm NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
VDD vdd 0 5
RD vdd d 10k
CL d 0 1p
VIN g 0 SIN(1.2 0.15 5Meg)
M1 d g 0 0 nm W=10u L=1u
.tran 40p 2u
.end
