include of a file that does not exist
* bench.id: rob_hs_include_missing_file
* bench.group: robustness
* bench.title: .include of a file that does not exist
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .include is ignored without any diagnostic (exit 0, a missing file does not even produce a warning); ngspice-47 aborts with 'Could not find include file'. Confirmed mechanism: an include of an EXISTING file with 'R9 2 0 1k' is ignored too (RustSpice v(2)=2.5 V, ngspice 1.667 V)
* bench.tags: hostile, include, silent_risk
* bench.est_rss_mib: 64
.include rob_hs_no_such_file.lib
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
