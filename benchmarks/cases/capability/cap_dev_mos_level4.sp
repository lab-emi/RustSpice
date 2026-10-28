NMOS LEVEL=4 card (BSIM1 VFB/PHI/K1/MUZ/TOX parameters)
* bench.id: cap_dev_mos_level4
* bench.group: capability
* bench.title: NMOS LEVEL=4 card (BSIM1 VFB/PHI/K1/MUZ/TOX parameters)
* bench.probes: i(vds)
* bench.expect: match
* bench.known_gap: silent-wrong-model-level
* bench.gap.note: LEVEL=4 (BSIM1) silently evaluated by the EKV path with default VTO/KP (not as level 1), exit 0: i(vds) = -0.308 mA at Vds=3 V vs ngspice -0.283 mA; max |diff| 8.5e-5 A (34 % at Vds=0.5 V); all BSIM1 parameters ignored
* bench.tags: mosfet, model_level, dc, silent_risk
* bench.note: expect=match per SUITE_SPEC 13.1 (ngspice computes this level, so the case flips to gap_closed when the level is implemented; a bare refusal would not close it and would be retagged unsupported-device-loud). BSIM1 is a compact analytic model; there is no independent oracle for it here, so the gap rests on the reference (tight ngspice) and on parameter insensitivity: stripping the BSIM1 parameters changes tight ngspice by 2.8e-4 A and RustSpice by exactly 0. Mechanism: every LEVEL>=2 selects one fixed EKV curve (with the same VTO/KP/LAMBDA the RustSpice output is bit-identical for LEVEL 2/3/4/6/8/14/49/54); on this deck it differs from RustSpice's own LEVEL=1 answer by 9.2e-5 A.
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=4 VFB=-0.5 PHI=0.8 K1=0.8 K2=0 ETA=0 MUZ=500 MUS=500 U0=0.1 U1=0.1 TOX=0.02 VDD=3)
.dc VDS 0 3 0.5
.end
