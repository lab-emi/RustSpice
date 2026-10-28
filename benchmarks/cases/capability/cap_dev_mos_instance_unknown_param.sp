MOSFET instance with an unknown parameter FOO=3
* bench.id: cap_dev_mos_instance_unknown_param
* bench.group: capability
* bench.title: MOSFET instance with an unknown parameter FOO=3
* bench.probes: i(v1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: unknown instance parameter FOO=3 is silently accepted and dropped: exit 0, empty stderr, i(v1) = -8.45e-4 A, identical to the card without FOO (ngspice: 'unknown parameter (foo)', exit 1)
* bench.tags: mosfet, card_syntax, ngspice_error_ok
* bench.note: ngspice rejects the card (`unknown parameter (foo)`, exit 1); RustSpice exits 0 and returns numbers (retagged from silent-ignore-model-param: malformed input, policy 13.1). Closed form for the card without FOO: 0.5*KP*W/L*(VGS-VTO)^2 = 0.5*1e-3*1.3^2 = 8.45e-4 A, reproduced by RustSpice and ngspice. M=2 on the same card is ignored by RustSpice (-8.45e-4 A) but doubled by ngspice (-1.69e-3 A): valid input dropped, a different defect (silent-ignore-multiplier)
V1 d 0 3
VG g 0 2
M1 d g 0 0 NM W=10u L=1u FOO=3
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u)
.op
.end
