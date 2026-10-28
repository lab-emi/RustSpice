PWL source (piecewise linear, five points)
* bench.id: cap_el_v_pwl
* bench.group: capability
* bench.title: PWL source (piecewise linear, five points)
* bench.probes: v(in)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: source, tran, waveform, pwl, silent_risk
* bench.note: Verified: ngspice-47 (tight reference) matches the piecewise-linear closed form 0,1,1,-1,0 V at 0,1,2,3,4 us to 2e-16 V; RustSpice stays at 0 V (error 1.0 V of the 2 V swing). The first PWL token is read as the DC value `PWL(0` -> not a number -> 0. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 PWL(0 0 1u 1 2u 1 3u -1 4u 0)
R1 in 0 1k
.tran 10n 5u
.end
