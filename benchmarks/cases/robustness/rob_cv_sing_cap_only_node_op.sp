Node connected only through two capacitors (floating at DC), .op
* bench.id: rob_cv_sing_cap_only_node_op
* bench.group: robustness
* bench.title: Node connected only through two capacitors (floating at DC), .op
* bench.probes: v(1)
* bench.expect: either
* bench.tags: ill_posed, floating, ngspice_error_ok, op
* bench.note: Ambiguous by design - consistent but non-unique: with both capacitors open the row of node 2 is all zeros and its right-hand side is 0 (rank A = rank [A|b] = 2), so ANY v(2) satisfies KCL and a regularised answer is legitimate (unlike the current-source cutsets, rob_cv_sing_isrc_*, which have no solution). ngspice-47 warns "singular matrix: check node 2" and reports v(2)=2.5 V; RustSpice returns v(2)=0 V silently (a missing diagnostic, not a wrong value). Probe is v(1) because the floating value is arbitrary.
V1 1 0 5
C1 1 2 1n
C2 2 0 1n
R1 1 0 1k
.op
.end
