1e-320 V (denormal) source across a 1k:1k divider
* bench.id: rob_hs_num_boundary_denormal_source
* bench.group: robustness
* bench.title: 1e-320 V (denormal) source across a 1k:1k divider
* bench.probes: v(1), v(2), i(v1)
* bench.tol.rel: 1e-2
* bench.tol.abs: 1e-322
* bench.floor: 1e-323
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: numeric, denormal, dc
* bench.est_rss_mib: 64
* bench.note: subnormal arithmetic end to end: v(1) = 1e-320, v(2) = 4.94e-321 and i(v1) = -4.94e-324 (one ulp, the smallest subnormal, which falls under the floor and is reported as vacuous); RustSpice and ngspice produce identical doubles. tol.abs 1e-322 (20 ulps) with floor 1e-323 keeps the comparison from being swallowed by the default 1e-9 / 1e-6; a flush-to-zero engine would fail it
V1 1 0 1e-320
R1 1 2 1k
R2 2 0 1k
.op
.end
