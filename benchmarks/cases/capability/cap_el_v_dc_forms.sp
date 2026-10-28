DC source forms: value only, dc, DC, dc+ac, value+ac+phase, ac only
* bench.id: cap_el_v_dc_forms
* bench.group: capability
* bench.title: DC source forms: value only, dc, DC, dc+ac, value+ac+phase, ac only
* bench.probes: v(n1), v(n2), v(n3), v(n4), v(n5), v(n6), v(n7)
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: source, op, parser
* bench.note: Every node is 3.3 V; V6 has no DC value (an AC-only source is 0 V at DC) and sits on top of V7.
V1 n1 0 3.3
V2 n2 0 dc 3.3
V3 n3 0 DC 3.3
V4 n4 0 dc 3.3 ac 1
V5 n5 0 3.3 ac 1 45
V6 n6 n7 ac 1
V7 n7 0 3.3
R1 n1 0 1k
R2 n2 0 1k
R3 n3 0 1k
R4 n4 0 1k
R5 n5 0 1k
R6 n6 0 1k
R7 n7 0 1k
.op
.end
