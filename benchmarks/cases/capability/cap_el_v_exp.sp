EXP source (rise and fall exponentials)
* bench.id: cap_el_v_exp
* bench.group: capability
* bench.title: EXP source (rise and fall exponentials)
* bench.probes: v(in)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.known_gap: silent-ignore-source-waveform
* bench.gap.note: EXP is not parsed: the source becomes DC 0 V without any diagnostic
* bench.tags: source, tran, waveform, exp, silent_risk
* bench.note: Verified: ngspice-47 matches v = V1+(V2-V1)(1-exp(-(t-TD1)/TAU1)) for TD1 < t <= TD2 and the added (V1-V2)(1-exp(-(t-TD2)/TAU2)) term after TD2 to 2e-16 V (V1=0, V2=1, TD1=1u, TAU1=300n, TD2=2u, TAU2=500n); RustSpice stays at 0 V (error 0.96 V). ngspice-47 .op of an EXP source is V1.
V1 in 0 EXP(0 1 1u 300n 2u 500n)
R1 in 0 1k
.tran 10n 5u
.end
