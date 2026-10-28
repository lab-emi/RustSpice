subcircuit used before its definition, ground inside the body
* bench.id: rob_hs_x_forward_ref_global_ground
* bench.group: robustness
* bench.title: X call placed before its .subckt definition; the body connects to the global ground node 0
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: subckt, dc, parser, lenient_ok
* bench.est_rss_mib: 64
* bench.note: node 0 inside the body is the global ground, not a local node: R3 (2k to ground) is in parallel with R2 (1k), so v(2) = 5 V * 667/(1000+667) = 2.0 V and i(v1) = -3 mA; a flattener that prefixed node 0 would leave R3 floating (v(2) = 2.5 V)
V1 1 0 5
X1 1 2 foo
R2 2 0 1k
.subckt foo a b
R1 a b 1k
R3 b 0 2k
.ends
.op
.end
