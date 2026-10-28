subcircuit that instantiates itself
* bench.id: rob_hs_sub_self_recursive
* bench.group: robustness
* bench.title: subcircuit that instantiates itself
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: hostile, subckt, recursion, resource, ngspice_crash
* bench.est_rss_mib: 400
* bench.note: Independent call-graph DFS finds the cycle loop -> loop, so no finite expansion exists and refusal is the only correct outcome. ngspice-47 prints "unknown subckt" at nesting depth 22 and then segfaults (exit 139 under -r, as in the harness; exit 1 without -r). est_rss_mib covers the observed abort (stack-bound, deterministic); a refusing run needs far less. Same mechanism as cap_lang_sub_recursive_direct (R1 and X1 in the other order: 375 vs 725 MiB at abort).
.subckt loop a b
X1 a b loop
R1 a b 1k
.ends
V1 1 0 5
X1 1 0 loop
.op
.end
