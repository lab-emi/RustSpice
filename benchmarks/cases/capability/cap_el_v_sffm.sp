SFFM source (single-frequency FM)
* bench.id: cap_el_v_sffm
* bench.group: capability
* bench.title: SFFM source (single-frequency FM)
* bench.probes: v(in), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.tags: source, tran, waveform, sffm, silent_risk
* bench.note: Verified: ngspice-47 matches v = vo + va*sin(2*pi*fc*t + mdi*sin(2*pi*fs*t)) = sin(2*pi*5e6*t + 3*sin(2*pi*500e3*t)) exactly; RustSpice stays at 0 V (error 1.0 V). The .op value of an SFFM source in ngspice-47 is 0 V (not vo), so this deck only tests the transient. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 SFFM(0 1 5meg 3 500k)
R1 in 0 1k
.tran 10n 5u
.end
