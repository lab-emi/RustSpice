Inductor with both terminals on the same node
* bench.id: cap_el_self_loop_inductor
* bench.group: capability
* bench.title: Inductor with both terminals on the same node
* bench.probes: v(a)
* bench.expect: either
* bench.ref: none
* bench.tags: passive, op, self_loop, singular, ngspice_error_ok, ambiguous
* bench.note: AMBIGUOUS, nobody is judged. The inductor branch equation v(a)-v(a) = 0 does not determine i(l1) at DC, so the matrix is structurally singular. ngspice-47 prints singular-matrix warnings through dynamic gmin, true gmin and source stepping, then falls back to its pseudo-transient operating point and still exits 0 with v(a) = 5 V, i(l1) = 0 and i(v1) = -5 mA (the zero is an arbitrary choice for the indeterminate current; the earlier note that ngspice gives no result was wrong for this version). RustSpice refuses with SingularMatrix and exit 1. The probed v(a) = 5 V is well defined, but i(l1) is not, so both a refusal and the 5 V answer are defensible; the harness takes no side (tag ngspice_error_ok lets the ngspice singular-matrix warnings through).
V1 a 0 5
R1 a 0 1k
L1 a a 1m
.op
.end
