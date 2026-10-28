SIN phase argument at .op (voltage and current sources, with and without delay)
* bench.id: cap_el_v_sin_phase_op
* bench.group: capability
* bench.title: SIN phase argument at .op (voltage and current sources, with and without delay)
* bench.probes: v(a), v(b), v(c), v(d)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.floor: 1e-6
* bench.expect: match
* bench.tags: source, op, waveform, sin, phase, silent_risk
* bench.note: ngspice-47 evaluates SIN(vo va freq td theta phase) at t=0 as vo + va*sin(2*pi*phase/360), also while the delay td is pending: V1 = 1+2*sin(90 deg) = 3, V2 = 0+1*sin(30 deg) = 0.5, V3 (delay 10n pending) = 3, I1 = 1m+2m*sin(90 deg) = 3 mA. Hand check: 3, 0.5, 3, 3 V. v(a), v(b) and v(d) have no delay, so their mismatch depends on no convention; v(c) (delay pending) relies on ngspice's pre-delay value vo + va*sin(phase), which could not be checked against another simulator. With all phases set to 0 RustSpice equals ngspice [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 a 0 SIN(1 2 1meg 0 0 90)
R1 a 0 1k
V2 b 0 SIN(0 1 1meg 0 0 30)
R2 b 0 1k
V3 c 0 SIN(1 2 1meg 10n 0 90)
R3 c 0 1k
I1 0 d SIN(1m 2m 1meg 0 0 90)
R4 d 0 1k
.op
.end
