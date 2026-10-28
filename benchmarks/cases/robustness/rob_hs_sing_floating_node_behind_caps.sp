node reachable only through capacitors (floating at DC)
* bench.id: rob_hs_sing_floating_node_behind_caps
* bench.group: robustness
* bench.title: node reachable only through capacitors (floating at DC)
* bench.probes: v(1), v(2)
* bench.expect: either
* bench.tags: hostile, singular, dc, floating
* bench.est_rss_mib: 64
* bench.note: v(2) is undefined at DC (capacitors are open and the history that set the charge is not in the netlist): ngspice warns "singular matrix" and returns the capacitive-divider value 2.5 V (consistent with gmin across the capacitors), RustSpice returns 0 V (gmin to ground only) without a warning; both finite, nobody can be judged: ambiguous
V1 1 0 5
C1 1 2 1n
C2 2 0 1n
.op
.end
