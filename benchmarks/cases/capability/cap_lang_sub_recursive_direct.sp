Subcircuit that instantiates itself
* bench.id: cap_lang_sub_recursive_direct
* bench.group: capability
* bench.title: .subckt LOOP contains X1 ... LOOP: infinite expansion must be refused, not overflow the stack
* bench.probes: v(in)
* bench.expect: loud_error
* bench.est_rss_mib: 800
* bench.tags: linear, op, subckt, recursion, invalid_input, ngspice_error_ok, ngspice_crash
* bench.note: Independent call-graph DFS finds the cycle loop -> loop, so no finite expansion exists and refusal is the only correct outcome. With the X1 line removed the deck solves in both tools (v(in) = 1 V, i(v1) = -1 mA = -1 V/1k). ngspice-47 prints "unknown subckt" at nesting depth 22 and then segfaults (exit 139 under -r, as in the harness; exit 1 without -r). Same mechanism as rob_hs_sub_self_recursive; swapping the R1 and X1 cards here changes the memory at abort from 725 to 375 MiB (verified).
.subckt LOOP a b
R1 a b 1k
X1 a b LOOP
.ends LOOP
V1 in 0 DC 1
X9 in 0 LOOP
.op
.end
