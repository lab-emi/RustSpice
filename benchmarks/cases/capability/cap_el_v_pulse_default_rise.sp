PULSE with only the delay given: the rise time defaults to the .tran step
* bench.id: cap_el_v_pulse_default_rise
* bench.group: capability
* bench.title: PULSE with only the delay given: the rise time defaults to the .tran step
* bench.probes: v(a), v(b)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.tags: source, tran, waveform, pulse, defaults, silent_risk
* bench.note: ngspice-47 measured at tmax 1n: PULSE(0 1 1.003u) starts rising at the delay and reaches 1 V exactly one .tran step (10 ns) later (v>0.01 at 1.000 us and v>0.99 at 1.010 us for td = 1u; the delay is 1.003u here so the edge sits off the 10 ns grid), then stays high; this matches the SPICE3 default TR = TSTEP (from the SPICE3 convention, not from ngspice sources). v(b) writes the rise time explicitly (10n) and is the control: it matches. Minimal flip: PULSE(0 1 1.003u 10n) agrees in both tools. Reference limits (13.4.1): ngspice-47's default follows the .tran step (.tran 20n gives 0.35 V and .tran 5n gives 1.0 V at 1.01u, verified) and it also replaces an explicit TR = 0 by the step (PULSE(0 1 1.003u 0) gives 0.7 V at 1.01u), so the reference is trustworthy for the 3-argument form with the documented TR = TSTEP default only; its own PW default differs between the 3-argument form (stays high) and PULSE(0 1 td tr tf) (falls at once: v = 0 V at 2u, verified), so omitted trailing PULSE arguments beyond the rise time are not covered here. Slug: no registered slug names 'omitted waveform argument defaulted to 0 instead of TSTEP' (a proposed pulse-default-timing slug is not registered and not used); silent-misparse is the closest registered fit.
V1 a 0 PULSE(0 1 1.003u)
R1 a 0 1k
V2 b 0 PULSE(0 1 1.003u 10n)
R2 b 0 1k
.tran 10n 5u
.end
