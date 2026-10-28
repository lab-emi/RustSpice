SIN with only offset and amplitude (frequency defaults to 1/TSTOP)
* bench.id: cap_el_v_sin_min_args
* bench.group: capability
* bench.title: SIN with only offset and amplitude (frequency defaults to 1/TSTOP)
* bench.probes: v(in)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.tags: source, tran, waveform, sin, defaults, silent_risk
* bench.note: ngspice defaults the frequency to 1/tstop, here one cycle over 5 us. Verified: ngspice-47 equals sin(2*pi*t/5us) to 2e-15 V; RustSpice stays at 0 V because SIN needs three arguments. The 1/TSTOP default is ngspice's convention; a silent DC 0 V is not defensible under any default. SIN(0 1 200k) is read correctly by RustSpice (2.4e-8 of swing against ngspice), so the three-argument minimum is the cause [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 SIN(0 1)
R1 in 0 1k
.tran 10n 5u
.end
