NMOS LEVEL=3 card (THETA/VMAX/ETA/KAPPA semi-empirical parameters)
* bench.id: cap_dev_mos_level3
* bench.group: capability
* bench.title: NMOS LEVEL=3 card (THETA/VMAX/ETA/KAPPA semi-empirical parameters)
* bench.probes: i(vds)
* bench.expect: match
* bench.known_gap: silent-wrong-model-level
* bench.gap.note: LEVEL=3 silently evaluated by the EKV path (not as level 1), exit 0: i(vds) is flat at -0.650 mA from Vds=1.5 V while ngspice still rises to -0.660 mA at 3 V; max |diff| 1.26e-4 A (24 % at Vds=1 V); THETA/VMAX/ETA/KAPPA have no effect
* bench.tags: mosfet, model_level, dc, silent_risk
* bench.note: expect=match per SUITE_SPEC 13.1 (ngspice computes this level, so the case flips to gap_closed when the level is implemented; a bare refusal would not close it and would be retagged unsupported-device-loud). No independent closed-form oracle exists for the level-3 equations, so the gap rests on the reference (tight ngspice) and on parameter insensitivity: stripping THETA/VMAX/ETA/KAPPA changes tight ngspice by 2.9e-4 A and RustSpice by exactly 0. Mechanism: every LEVEL>=2 selects one fixed EKV curve (with the same VTO/KP/LAMBDA the RustSpice output is bit-identical for LEVEL 2/3/4/6/8/14/49/54); on this deck it differs from RustSpice's own LEVEL=1 answer by 1.95e-4 A.
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=3 VTO=0.7 KP=100u THETA=0.1 VMAX=1e5 ETA=0.05 KAPPA=0.1)
.dc VDS 0 3 0.5
.end
