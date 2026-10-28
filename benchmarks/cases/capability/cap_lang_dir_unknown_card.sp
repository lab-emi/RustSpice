Unknown dot card
* bench.id: cap_lang_dir_unknown_card
* bench.group: capability
* bench.title: .foobar 1 2 3 (not a SPICE directive) next to .op
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, op, invalid_input, ngspice_error_ok
* bench.note: ngspice-47 rejects the deck (exit 1, "unimplemented dot command '.foobar'", 'circuit not parsed'); retagged from silent-ignore-directive because the card is not a directive any simulator knows, so this is silent acceptance of input ngspice rejects (policy 13.1). The numbers equal the valid deck (8*1k/(3k+1k) = 2.0 V). Typos of valid analyses go through the same arm and are worse because the analysis is lost: `.dcc V1 0 8 2` is read as a single .op (v(out) = 2.0 V, one point) while ngspice rejects it and the valid `.dc V1 0 8 2` gives a 5-point sweep
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.foobar 1 2 3
.op
.end
