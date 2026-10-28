A .dc sweep followed by .op: the swept source must be restored to its DC value (3 V) for the operating point
* bench.id: acc_dc_multi_dc_then_op
* bench.group: accuracy
* bench.title: A .dc sweep followed by .op: the swept source must be restored to its DC value (3 V) for the operating point
* bench.probes: v(out)
* bench.expect: match
* bench.tags: linear, op, dc, multi_analysis, silent_risk
* bench.note: ngspice runs .op at the deck value V1 = 3 V; the sweep itself matches. Placing the .op before the .dc makes RustSpice report 1.5 V, so only the missing restore after the sweep matters
V1 in 0 3
R1 in out 1k
R2 out 0 1k
.dc V1 0 1 0.5
.op
.end
