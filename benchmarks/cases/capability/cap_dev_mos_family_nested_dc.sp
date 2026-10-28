NMOS L1 output-curve family: nested .dc Vds 0..4 / Vgs 1..3
* bench.id: cap_dev_mos_family_nested_dc
* bench.group: capability
* bench.title: NMOS L1 output-curve family: nested .dc Vds 0..4 / Vgs 1..3
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc, nested_sweep
* bench.note: evidence: RustSpice output equals the single sweep `.dc VDS 0 4 0.5` (9 points, VG stays at its card value); three separate RustSpice sweeps at Vgs=1,2,3 equal the 27-point ngspice nested sweep to 1e-14 A. Independent truth: the closed-form level-1 current (kp 100u, W/L 10, vto 0.7, lambda 0.02), tight ngspice is within 4.0e-12 A of it; no waveform or restore is involved [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
VDS d 0 0
VG g 0 1
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VDS 0 4 0.5 VG 1 3 1
.end
