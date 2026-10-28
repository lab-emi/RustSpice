Two subcircuits that instantiate each other
* bench.id: cap_lang_sub_recursive_mutual
* bench.group: capability
* bench.title: PING instantiates PONG and PONG instantiates PING: cycle must be refused
* bench.probes: v(in)
* bench.expect: loud_error
* bench.est_rss_mib: 800
* bench.tags: linear, op, subckt, recursion, invalid_input, ngspice_error_ok, ngspice_crash
* bench.note: Independent call-graph DFS finds the cycle ping -> pong -> ping, so no finite expansion exists and refusal is the only correct outcome. With the X1 line of PONG removed the deck solves in both tools (v(in) = 1 V, i(v1) = -1.5 mA = -1 V/(1k||2k)). ngspice-47 prints "unknown subckt" at nesting depth 22 and then segfaults (exit 139 under -r; exit 1 without -r). Same mechanism as rob_hs_sub_mutual_recursive, where R1 and X1 appear in the other order inside the subckts (375 vs 725 MiB at abort).
.subckt PING a b
R1 a b 1k
X1 a b PONG
.ends PING
.subckt PONG a b
R1 a b 2k
X1 a b PING
.ends PONG
V1 in 0 DC 1
X9 in 0 PING
.op
.end
