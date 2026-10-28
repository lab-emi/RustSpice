DC sweep followed by an operating point
* bench.id: cap_lang_multi_dc_then_op
* bench.group: capability
* bench.title: .dc V1 0 5 1 then .op: the .op must see V1 at its netlist value (3 V), not the last sweep value
* bench.probes: v(out)
* bench.tags: linear, multi_analysis, dc, op
* bench.note: The .dc block itself matches ngspice; only the .op after it is wrong. Truth: v(out) = V1/2 = 1.5 V at the netlist V1 = 3 V (ngspice 1.5 V, as-is and tight)
V1 in 0 DC 3
R1 in out 1k
R2 out 0 1k
.dc V1 0 5 1
.op
.end
