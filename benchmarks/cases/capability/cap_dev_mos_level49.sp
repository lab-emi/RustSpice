NMOS LEVEL=49 card (unsupported by RustSpice; must be refused)
* bench.id: cap_dev_mos_level49
* bench.group: capability
* bench.title: NMOS LEVEL=49 card (unsupported by RustSpice; must be refused)
* bench.probes: i(vds)
* bench.expect: loud_error
* bench.known_gap: silent-wrong-model-level
* bench.gap.note: LEVEL=49 (BSIM3v3) silently evaluated by the EKV path (not as level 1), exit 0: i(vds) = -0.308 mA at Vds=3 V instead of a refusal (ngspice -1.3e-5 A)
* bench.tags: mosfet, model_level, dc, unsupported
* bench.note: expect=loud_error is a recorded exception to SUITE_SPEC 13.1 (a level ngspice computes would normally be expect=match): there is no independent BSIM3v3 oracle here (13.4.1), so tight ngspice's numbers (|i(vds)| up to 1.3e-5 A) cannot be judged against truth, and the only defensible RustSpice outcomes are a real implementation or a refusal. ngspice gives the same numbers for LEVEL=8 and LEVEL=49 (VERSION=3.3.0), 23x below RustSpice. Today it returns numbers from its fixed EKV path (bit-identical for the LEVEL 4/8/14/49/54 decks, 9.2e-5 A away from its own LEVEL=1 answer), i.e. a different model is evaluated silently. Retag to expect: match when a real implementation lands.
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=49 VERSION=3.3.0)
.dc VDS 0 3 0.5
.end
