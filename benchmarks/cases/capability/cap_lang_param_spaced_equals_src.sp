Spaces around the equals sign of .param, used as a source value
* bench.id: cap_lang_param_spaced_equals_src
* bench.group: capability
* bench.title: .param va = 8 (spaces around '=') used as the source value V1 {va}
* bench.probes: v(in), v(out), i(v1)
* bench.expect: match
* bench.tags: linear, op, param, syntax, silent_risk
* bench.note: Silent twin of cap_lang_param_spaced_equals, whose resistor consumer makes RustSpice refuse loudly. ngspice-47 (tight) and an exact rational solve agree: v(in) = 8 V, v(out) = 8 V * 1k/(3k+1k) = 2 V, i(v1) = -8 V/4k = -2 mA. Minimal flip: `.param va=8` without spaces is read correctly by RustSpice (8 V, 2 V). Slug silent-misparse: valid .param syntax is read as something else without any diagnostic. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
.param va = 8
V1 in 0 DC {va}
R1 in out 3k
R2 out 0 1k
.op
.end
