save of a node that does not exist (plus a valid one)
* bench.id: rob_hs_probe_save_missing_node
* bench.group: robustness
* bench.title: .save of a node that does not exist (plus a valid one)
* bench.probes: v(2)
* bench.expect: match
* bench.tags: hostile, probe, directive, lenient_ok
* bench.est_rss_mib: 64
* bench.note: ngspice restricts the output to the saved v(2); RustSpice ignores .save and prints everything; only v(2) is compared
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.save v(nosuch) v(2)
.op
.end
