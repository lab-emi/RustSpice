NMOS output characteristic at VGS=1.5 V, 2049 points
* bench.id: perf_dc_nmos_output_2k
* bench.group: performance
* bench.title: NMOS output characteristic at VGS=1.5 V, 2049 points
* bench.probes: i(vd), v(d)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 4
* bench.est_rss_mib: 10
* bench.tags: nonlinear, dc, sweep, mos, jit_tier
* bench.note: triode-to-saturation transition of a level-1 MOSFET: the Jacobian changes region mid-sweep; two voltage sources and one device; step 2^-10 V
.model nm NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
VG g 0 1.5
VD d 0 0
M1 d g 0 0 nm W=10u L=1u
.dc VD 0 2 0.0009765625
.end
