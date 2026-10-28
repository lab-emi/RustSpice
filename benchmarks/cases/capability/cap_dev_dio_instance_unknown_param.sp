Diode instance with an unknown parameter FOO=3
* bench.id: cap_dev_dio_instance_unknown_param
* bench.group: capability
* bench.title: Diode instance with an unknown parameter FOO=3
* bench.probes: i(v1)
* bench.expect: loud_error
* bench.tags: diode, card_syntax, ngspice_error_ok
* bench.note: ngspice rejects the card (`unknown parameter (foo)`, exit 1); RustSpice exits 0 and returns numbers (retagged from silent-ignore-model-param: malformed input, policy 13.1). Variants (both tools run): the valid AREA=3 is honoured by RustSpice and ngspice alike (i(v1) = -1.7011040e-2 A), so only the unknown name is dropped; M=3 is also ignored by RustSpice, but that is valid input (silent-ignore-multiplier), not this deck's defect [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 a 0 0.7
D1 a 0 DM FOO=3
.model DM D(IS=1e-14)
.op
.end
