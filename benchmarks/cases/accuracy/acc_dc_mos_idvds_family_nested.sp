NMOS id-vds family from a nested .dc: Vds 0..3 V (inner) for Vgs = 1, 1.5, 2 V (outer)
* bench.id: acc_dc_mos_idvds_family_nested
* bench.group: accuracy
* bench.title: NMOS id-vds family from a nested .dc: Vds 0..3 V (inner) for Vgs = 1, 1.5, 2 V (outer)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.known_gap: silent-misparse
* bench.gap.note: nested .dc: the .dc parser reads only the first sweep and drops the rest of the card without a message; RustSpice returns 7 points at the deck Vgs = 0 (|i(vds)| <= 3e-12 A) instead of the 21-point family
* bench.size: 21
* bench.tags: mos, level1, dc, sweep, nested_sweep
* bench.note: ngspice returns the 21-point two-dimensional sweep, inner variable first; tol.abs 1e-10 (V and A): the default 1e-9 would be 1e-5 relative on 100 uA currents. Independent truth: the closed-form level-1 current (kp 100u, W/L 20, vto 0.7, lambda 0.03): tight ngspice is within 3.0e-12 A of it (about 1e-12 S times Vds). Three separate RustSpice single sweeps at Vgs = 1, 1.5 and 2 equal the three ngspice blocks to 1e-14 A, so the device model is right and only the dropped second sweep fails; no waveform or restore is involved
Vgs g 0 0
Vds d 0 0
M1 d g 0 0 nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.dc Vds 0 3 0.5 Vgs 1 2 0.5
.end
