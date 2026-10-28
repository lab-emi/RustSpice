Diode connected straight across a 5 V source (no series resistance): exp(193) current
* bench.id: rob_cv_diode_direct_5v_op
* bench.group: robustness
* bench.title: Diode connected straight across a 5 V source (no series resistance): exp(193) current
* bench.probes: i(v1)
* bench.expect: match
* bench.tags: nonlinear, diode, overflow, silent_risk, op
* bench.note: Physically meaningless operating point (an ideal 5 V source across an ideal diode with no series resistance) but mathematically well posed: the unique solution of the stated model is i = IS*(exp(5/Vt)-1) + gmin*5 = 9.00231949409627e69 A (50-digit mpmath, Vt=0.025864917007155805), well inside the f64 range. ngspice-47 returns -9.002319493985e69 A (relative difference 1.2e-11, as-is and tight identical), RustSpice returns exactly IS*exp(80) = 5.540622384394e20 A. Decision: expect=match, because the reference is exact for the model and a refusal would also have been acceptable but silent saturation is not; both tools reject the 1 kV variant (rob_cv_diode_overflow_1kv_op) where exp overflows. v(1)=5 V is trivially equal and excluded (probe is the source current).
V1 1 0 5
D1 1 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
