PWL source with r=0 (repeat from the start)
* bench.id: cap_el_v_pwl_repeat
* bench.group: capability
* bench.title: PWL source with r=0 (repeat from the start)
* bench.probes: v(in)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.tags: source, tran, waveform, pwl, silent_risk
* bench.note: ngspice-47 syntax: r=<time> repeats the sequence from that time point. Verified: ngspice reproduces the closed-form triangle (period 2 us, 0..1 V) to 1e-16 V; RustSpice stays at 0 V. r= is an ngspice-specific extension; the one-shot PWL without r=0 is equally DC 0 V in RustSpice (see cap_el_v_pwl), so the defect is the dropped PWL card, not the repeat option [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 PWL(0 0 1u 1 2u 0) r=0
R1 in 0 1k
.tran 10n 5u
.end
