dc V1 0 5 -1 (negative step on an ascending range)
* bench.id: rob_hs_dc_wrong_direction_step
* bench.group: robustness
* bench.title: .dc V1 0 5 -1 (negative step on an ascending range)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: either
* bench.tags: hostile, dc, args
* bench.est_rss_mib: 64
* bench.note: ngspice exits 0 but writes an empty sweep (0 points, no error text); RustSpice silently sweeps 0..5 upwards (6 points), i.e. it reinterprets the sign of the step. A descending step on an ascending range is undefined by the language: ambiguous (the engine labels the empty ngspice rawfile ngspice_only_error)
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.dc V1 0 5 -1
.end
