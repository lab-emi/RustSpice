Polynomial VCCS G ... POLY(1) (nonlinear controlled source)
* bench.id: cap_el_g_poly_misparse
* bench.group: capability
* bench.title: Polynomial VCCS G ... POLY(1) (nonlinear controlled source)
* bench.probes: v(out)
* bench.expect: match
* bench.known_gap: silent-misparse
* bench.gap.note: `poly(1)` is read as the name of a control node and the next token (the control node `in`) as the transconductance, so the card becomes a VCCS of gain 0 controlled by a new floating node poly(1): v(out) = 0 V and exit 0, with no diagnostic (ngspice and the hand value give 4 V)
* bench.tags: controlled, op, vccs, poly, nonlinear, silent_risk
* bench.note: Hand value: i = p0 + p1*v + p2*v^2 = 0 + 1m*2 + 0.5m*4 = 4 mA into out, v(out) = 4 V; ngspice-47 (tight reference) returns exactly 4. Re-verified: RustSpice creates an extra node named poly(1) (v(poly(1)) = 0 V is printed) and returns v(out) = 0 V. Minimal flip: the plain linear card `G1 0 out in 0 1m` is read correctly (v(out) = 2 V in both tools). Expectation is match because a silent wrong answer is never the correct behaviour; a loud refusal would also violate it and would remain known_gap_confirmed.
V1 in 0 2
G1 0 out poly(1) in 0 0 1m 0.5m
R1 out 0 1k
.op
.end
