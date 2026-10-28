PULSE with the pulse width omitted but the fall time given
* bench.id: cap_el_v_pulse_omitted_args
* bench.group: capability
* bench.title: PULSE with the pulse width omitted but the fall time given
* bench.probes: v(b), v(c), v(d)
* bench.tol.norm: 5e-3
* bench.expect: either
* bench.tags: source, tran, waveform, pulse, defaults, ambiguous
* bench.note: AMBIGUOUS, nobody is judged for v(c). ngspice-47 measured at tmax 1n: PULSE(0 1 1u 100n 200n) (fall time given, width omitted) rises over 100 ns and then falls at once, as if the width default were 0, and is back at 0 by 1.3 us. RustSpice keeps the omitted width at 1e9 s, so v(c) stays at 1 V for the whole run (normalised error about 1 against the ngspice trace). The SPICE3 convention as we know it is PW default = TSTOP, which is what RustSpice does and what ngspice does itself when the fall time is omitted too (v(b) stays high in both); with no specification both tools follow, the deck records the divergence without declaring a winner. v(b) (rise time only) and the fully specified v(d) are the agreeing anchors.
V2 b 0 PULSE(0 1 1u 100n)
R2 b 0 1k
V3 c 0 PULSE(0 1 1u 100n 200n)
R3 c 0 1k
V4 d 0 PULSE(0 1 1u 100n 200n 1u)
R4 d 0 1k
.tran 10n 5u
.end
