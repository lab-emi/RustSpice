Unknown option name
* bench.id: cap_lang_opt_unknown_name
* bench.group: capability
* bench.title: .options foobarbaz=3 (not an option of any simulator) next to a valid reltol
* bench.probes: v(out)
* bench.tags: linear, op, options, invalid_input
* bench.note: ngspice-47 accepts the unknown name silently, so RustSpice must at least not fail
.options reltol=1e-4 foobarbaz=3
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.op
.end
