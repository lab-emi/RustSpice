PULSE whose period is shorter than rise + width + fall (degenerate)
* bench.id: cap_el_v_pulse_period_lt_width
* bench.group: capability
* bench.title: PULSE whose period is shorter than rise + width + fall (degenerate)
* bench.probes: v(a), v(b)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.tags: source, tran, waveform, pulse, degenerate
* bench.note: V1 has per 46.1n < tr+pw+tf = 120n and also < pw: every period restarts at v1 before the fall begins, so the waveform is a 10 ns rise to v2, held until the period ends, then a jump back to v1. V2 has per 25n < tr = 30n: it never reaches v2 (peak 1.667 V) and restarts its rise every period. No manual specifies this case; both simulators use the same convention (time modulo period, then the trapezoid), verified with ngspice at tmax 0.05n against that formula to 1e-15 away from the jumps. All jump instants (k*46.1n, 3n+k*25n) are at least 0.5 ns from the 5 ns comparison grid so no sample sits on a discontinuity.
V1 a 0 PULSE(0 1 0 10n 10n 100n 46.1n)
R1 a 0 1k
V2 b 0 PULSE(0 2 3n 30n 5n 4n 25n)
R2 b 0 1k
.tran 5n 400n
.end
