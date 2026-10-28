I source value equal to the smallest normal double (2.2250738585072014e-308) into 1k
* bench.id: rob_hs_num_boundary_i_dblmin_literal
* bench.group: robustness
* bench.title: I source value equal to the smallest normal double (2.2250738585072014e-308) into 1k
* bench.probes: v(1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-310
* bench.floor: 1e-310
* bench.expect: either
* bench.tags: numeric, denormal, dc, ngspice_own_error
* bench.est_rss_mib: 64
* bench.note: exact answer v(1) = I*1k/(1+1k*gmin) = 2.2250738e-305 V (RustSpice). ngspice-47 returns v(1) = 0 for this literal and for 2.2250738585072011e-308 while 2.2e-308, 1e-308 and 1e-320 work: a number-parsing quirk of ngspice at the DBL_MIN literal, so the reference is wrong here and the case is informational (info / mismatch); not a RustSpice defect
I1 0 1 2.2250738585072014e-308
R1 1 0 1k
.op
.end
