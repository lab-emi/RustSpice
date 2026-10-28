two subcircuits that instantiate each other
* bench.id: rob_hs_sub_mutual_recursive
* bench.group: robustness
* bench.title: two subcircuits that instantiate each other
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: hostile, subckt, recursion, resource, ngspice_crash
* bench.est_rss_mib: 400
* bench.note: Independent call-graph DFS finds the cycle sa -> sb -> sa, so no finite expansion exists and refusal is the only correct outcome. ngspice-47 also segfaults on this deck (exit 139 under -r, as in the harness; "unknown subckt" and exit 1 without -r). est_rss_mib covers the observed abort (stack-bound, deterministic); a refusing run needs far less. Same mechanism as cap_lang_sub_recursive_mutual (R1 and X1 in the other order: 375 vs 725 MiB at abort).
.subckt sa a b
X1 a b sb
R1 a b 1k
.ends
.subckt sb a b
X1 a b sa
R1 a b 1k
.ends
V1 1 0 5
Xt 1 0 sa
.op
.end
