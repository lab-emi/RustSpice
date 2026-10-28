SIN with phase, damping and delay at several time points
* bench.id: cap_el_v_sin_phase_tran
* bench.group: capability
* bench.title: SIN with phase, damping and delay at several time points
* bench.probes: v(a), v(b), v(c), v(d)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.tags: source, tran, waveform, sin, phase, damping, delay, silent_risk
* bench.note: ngspice-47: V = vo + va*exp(-(t-td)*theta)*sin(2*pi*f*(t-td) + 2*pi*phase/360) for t > td, and vo + va*sin(2*pi*phase/360) before td. V1 phase 90 (a cosine), V2 phase -30 with delay 0.4u, V3 damped (theta 1.5meg) with phase 45 and delay 0.25u, V4 plain phase 180. The source nodes are probed directly so any difference is waveform evaluation, not integration. Verified against the closed form in Python (ngspice to 1e-15). v(a) and v(d) have no delay and are unambiguous; the pre-delay segments of v(b) and v(c) rely on ngspice's pre-delay value vo + va*sin(phase). With all phases set to 0 the largest RustSpice-vs-ngspice difference is 5.4e-4 of swing (below tol.norm 2e-3), so damping and delay are handled and only the phase is the cause [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 a 0 SIN(0 1 2meg 0 0 90)
R1 a 0 1k
V2 b 0 SIN(0.5 1 2meg 0.4u 0 -30)
R2 b 0 1k
V3 c 0 SIN(0 2 2meg 0.25u 1.5meg 45)
R3 c 0 1k
V4 d 0 SIN(1 1 1meg 0 0 180)
R4 d 0 1k
.tran 5n 2u
.end
