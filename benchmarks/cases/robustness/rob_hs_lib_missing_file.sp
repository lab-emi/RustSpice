lib of a file that does not exist
* bench.id: rob_hs_lib_missing_file
* bench.group: robustness
* bench.title: .lib of a file that does not exist
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, include, silent_risk
* bench.est_rss_mib: 64
.lib rob_hs_no_such_file.lib typ
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
