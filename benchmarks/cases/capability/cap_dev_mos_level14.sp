NMOS LEVEL=14 card (unsupported by RustSpice; must be refused)
* bench.id: cap_dev_mos_level14
* bench.group: capability
* bench.title: NMOS LEVEL=14 card (unsupported by RustSpice; must be refused)
* bench.probes: i(vds)
* bench.expect: loud_error
* bench.tags: mosfet, model_level, dc, unsupported
* bench.note: expect=loud_error is a recorded exception to SUITE_SPEC 13.1 (a level ngspice computes would normally be expect=match): there is no independent BSIM4 oracle here (13.4.1), so tight ngspice's numbers (|i(vds)| up to 4.7e-3 A) cannot be judged against truth, and the only defensible RustSpice outcomes are a real implementation or a refusal. ngspice gives the same numbers for LEVEL=14 and LEVEL=54 (VERSION=4.8.0), 15x above RustSpice. Today it returns numbers from its fixed EKV path (bit-identical for the LEVEL 4/8/14/49/54 decks, 9.2e-5 A away from its own LEVEL=1 answer), i.e. a different model is evaluated silently. Retag to expect: match when a real implementation lands. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=14 VERSION=4.8.0)
.dc VDS 0 3 0.5
.end
