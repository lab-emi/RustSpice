Select one section of a library file
* bench.id: cap_lang_inc_lib_section
* bench.group: capability
* bench.title: .lib cap_lang_inc_lib_sections.lib FF picks the middle of three corner sections (IS=1e-12, N=1.3)
* bench.probes: v(out)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .lib file section ignored: model DCORNER undefined, diode silently uses defaults
* bench.tags: nonlinear, op, include, lib_section
* bench.note: the library path is relative to the deck's directory; TT/FF/SS define the same model name with different parameters
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DCORNER
.lib cap_lang_inc_lib_sections.lib FF
.op
.end
