NMOS differential pair transfer curve, 2049 points
* bench.id: perf_dc_diffpair_2k
* bench.group: performance
* bench.title: NMOS differential pair transfer curve, 2049 points
* bench.probes: v(d1), v(d2), i(vdd)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 9
* bench.est_rss_mib: 10
* bench.tags: nonlinear, dc, sweep, mos, amplifier, jit_tier
* bench.note: source-coupled pair with resistor tail swept from one input far below the other to far above it; smooth balanced-to-switched transition; step 2^-11 V
.model nm NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
VDD vdd 0 3.3
VCM in2 0 1.5
VIN in1 0 1
RD1 vdd d1 10k
RD2 vdd d2 10k
RT tail 0 2k
M1 d1 in1 tail 0 nm W=20u L=1u
M2 d2 in2 tail 0 nm W=20u L=1u
.dc VIN 1 2 0.00048828125
.end
