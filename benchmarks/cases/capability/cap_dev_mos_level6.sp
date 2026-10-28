NMOS LEVEL=6 card (MOS6 KC/NV/NVTH/PS parameters)
* bench.id: cap_dev_mos_level6
* bench.group: capability
* bench.title: NMOS LEVEL=6 card (MOS6 KC/NV/NVTH/PS parameters)
* bench.probes: i(vds)
* bench.expect: match
* bench.known_gap: silent-wrong-model-level
* bench.gap.note: LEVEL=6 silently evaluated by the EKV path (not as level 1), exit 0: i(vds) = -0.130 mA at Vds=3 V vs ngspice -1.30 mA (10x); max |diff| 1.17e-3 A; KC has no effect
* bench.tags: mosfet, model_level, dc, silent_risk
* bench.note: expect=match per SUITE_SPEC 13.1 (ngspice computes this level, so the case flips to gap_closed when the level is implemented; a bare refusal would not close it and would be retagged unsupported-device-loud). No independent closed-form oracle exists for the level-6 equations, so the gap rests on the reference (tight ngspice) and on parameter insensitivity: removing KC=100u changes tight ngspice by 6.5e-4 A and RustSpice by exactly 0. Mechanism: every LEVEL>=2 selects one fixed EKV curve (with the same VTO/KP/LAMBDA the RustSpice output is bit-identical for LEVEL 2/3/4/6/8/14/49/54); on this deck it differs from RustSpice's own LEVEL=1 answer by 3.9e-5 A.
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=6 VTO=0.7 KC=100u NV=0.5 NVTH=0.5 PS=0.1)
.dc VDS 0 3 0.5
.end
