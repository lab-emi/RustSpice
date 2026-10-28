.param card inside a subcircuit body, used as a source value
* bench.id: cap_lang_param_inside_subckt_src
* bench.group: capability
* bench.title: .param vv=3 inside the body of .subckt SRC is used by V1 {vv}
* bench.probes: v(out), v(n)
* bench.expect: match
* bench.known_gap: silent-ignore-directive
* bench.gap.note: the .param card inside the .subckt body is not recorded and the undefined {vv} is silently read as 0 V: RustSpice exits 0 with v(out) = v(n) = 0 V where ngspice gives 3 V and 2 V
* bench.tags: linear, op, param, subckt, silent_risk
* bench.note: Silent twin of cap_lang_param_inside_subckt, whose resistor consumer makes RustSpice refuse loudly. ngspice-47 (tight) and an exact rational solve agree: v(out) = 3 V, v(n) = 3 V * 2k/(1k+2k) = 2 V. Minimal flip: the same .param card at top level is read correctly by RustSpice (3 V, 2 V). Slug silent-ignore-directive: a supported dot card is accepted and ignored in the subcircuit scope and the undefined reference defaults silently.
.subckt SRC out
.param vv=3
V1 out 0 DC {vv}
.ends SRC
X1 out SRC
R1 out n 1k
R2 n 0 2k
.op
.end
