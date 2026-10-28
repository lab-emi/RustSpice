Behavioural current source B (I = 1m*v(in)^2)
* bench.id: cap_el_b_current_nonlinear
* bench.group: capability
* bench.title: Behavioural current source B (I = 1m*v(in)^2)
* bench.probes: v(out)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the B card is refused at parse time (`Unsupported device card B1`, exit 1, no panic) where ngspice computes v(out) = 4 V
* bench.tags: controlled, op, behavioural, nonlinear, unsupported
* bench.note: Hand value: I = 1m*v(in)^2 = 1m*4 = 4 mA pushed into out, v(out) = 4 V; ngspice-47 returns exactly 4. A feature ngspice supports and RustSpice refuses loudly is recorded as expect=match + unsupported-device-loud (SUITE_SPEC 13.1), so the case flips to gap_closed once B is implemented.
V1 in 0 2
B1 0 out I = 1m*v(in)*v(in)
R1 out 0 1k
.op
.end
