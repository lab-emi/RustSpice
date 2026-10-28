Include a model file next to the deck
* bench.id: cap_lang_inc_file
* bench.group: capability
* bench.title: .include cap_lang_inc_file_models.lib supplies the diode model DINC (IS=1e-12, N=1.5)
* bench.probes: v(out)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .include ignored: model DINC undefined, diode silently uses defaults (0.693 V instead of 0.859 V)
* bench.tags: nonlinear, op, include
* bench.note: the include path is relative to the deck's directory
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DINC
.include cap_lang_inc_file_models.lib
.op
.end
