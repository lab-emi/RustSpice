both subcircuit ports bound to ground
* bench.id: rob_hs_x_ports_aliased_to_ground
* bench.group: robustness
* bench.title: both subcircuit ports bound to ground
* bench.probes: v(1), i(v1)
* bench.expect: match
* bench.tags: subckt, dc, lenient_ok
* bench.est_rss_mib: 64
* bench.note: ports a and b of the subcircuit are both bound to ground (X1 0 0 1 s), so R1 (1k) and R2 (2k) end up in parallel between node 1 and ground together with R9 (1k): i(v1) = -(5/1k + 5/1k + 5/2k) = -12.5 mA. Redesigned in calibration: the earlier version aliased the ports to node 1, where the subcircuit carried no current and a dropped X1 gave the same numbers
.subckt s a b c
R1 a c 1k
R2 b c 2k
.ends
V1 1 0 5
X1 0 0 1 s
R9 1 0 1k
.op
.end
