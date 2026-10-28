MOSFET card with only three nodes (bulk missing): malformed
* bench.id: cap_dev_mos_card_missing_bulk
* bench.group: capability
* bench.title: MOSFET card with only three nodes (bulk missing): malformed
* bench.probes: i(v1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: 3-node M card accepted: NM is read as the bulk node (V(NM) = 0 V) and W=10u as the model name, which is undefined and silently defaults, so W=10u is lost: exit 0, i(v1) = -4.000e-3 A against -8.45e-4 A for the valid 4-node card
* bench.tags: mosfet, card_syntax, ngspice_error_ok
* bench.note: ngspice rejects the card (`not enough nodes`, exit 1); RustSpice takes the model name NM as a fourth node (V(NM) appears in its output, 0 V) and exits 0 (retagged from silent-ignore-directive: malformed input, policy 13.1). Arity is checked by raw token count (at least 6 tokens), not by positional fields: the 5-token card `M1 d g 0 NM` is refused by RustSpice (ParseError, exit 1), adding W=10u makes it silent. Closed form for the RustSpice reading (undefined model, so the defaults KP=2e-5, VTO=0, W=100u apply, L=1u, VGS=2 V): 1e-5*100*4 = 4.0e-3 A, which ngspice also returns for a bare `.model NM NMOS` card; valid 4-node card with KP=100u, VTO=0.7, W=10u: 0.5*1e-3*1.3^2 = 8.45e-4 A, reproduced by RustSpice and ngspice. The mechanism overlaps with silent-ignore-directive (an undefined .model silently defaults)
V1 d 0 3
VG g 0 2
M1 d g 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u)
.op
.end
