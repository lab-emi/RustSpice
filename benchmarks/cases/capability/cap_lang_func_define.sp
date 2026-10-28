User-defined function
* bench.id: cap_lang_func_define
* bench.group: capability
* bench.title: .func dbl(x) {2*x} used in an element value
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: Invalid resistance value `{dbl(1k)}` (the .func card itself is ignored silently like any unknown dot card; only the resistor consumer makes the deck loud)
* bench.tags: linear, op, param, func
* bench.note: ngspice-47 computes v(out) = 6 V * 1k/(2k+1k) = 2 V (hand-checked)
.func dbl(x) {2*x}
V1 in 0 DC 6
R1 in out {dbl(1k)}
R2 out 0 1k
.op
.end
