Include that includes another file
* bench.id: cap_lang_inc_nested
* bench.group: capability
* bench.title: cap_lang_inc_nested_a.lib includes cap_lang_inc_nested_b.lib which defines DNEST
* bench.probes: v(out)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .include ignored: model DNEST undefined, diode silently uses defaults
* bench.tags: nonlinear, op, include
* bench.note: include paths are relative to the deck's directory
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DNEST
.include cap_lang_inc_nested_a.lib
.op
.end
