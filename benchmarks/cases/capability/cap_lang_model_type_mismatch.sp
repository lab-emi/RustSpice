Diode instance with a MOSFET model
* bench.id: cap_lang_model_type_mismatch
* bench.group: capability
* bench.title: D1 refers to a model declared NMOS: the model type does not match the element
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: nonlinear, op, model, invalid_input, ngspice_error_ok
* bench.note: RustSpice looks diode models up in a diode-only table, so the wrong-type model is a lookup miss and the diode silently takes its defaults; v(out) = 0.6928875986 V is identical to an undefined model name and to a bare `.model MN D` (ngspice agrees on the bare D model, which it accepts; it rejects the undefined name). so the NMOS card contributes nothing to the diode (a valid D(IS=1e-12) gives 0.5744767317 V in both tools). Independent mpmath solve of the default diode (IS=1e-14, N=1, CODATA constants, T=300.15 K): 0.69288783 V, 2.3e-7 V above both tools (the same offset in both, immaterial here). The mechanism overlaps with silent-ignore-directive (undefined .model refs default silently); kept under lenient-malformed-input because the model IS defined, only with the wrong type (policy 13.1) [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 DC 5
R1 in out 1k
D1 out 0 MN
.model MN NMOS(VTO=0.7 KP=100u)
.op
.end
