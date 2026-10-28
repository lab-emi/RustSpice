.param card inside a subcircuit body, used as a source value
* bench.id: cap_lang_param_inside_subckt_src
* bench.group: capability
* bench.title: .param vv=3 inside the body of .subckt SRC is used by V1 {vv}
* bench.probes: v(out), v(n)
* bench.expect: match
* bench.tags: linear, op, param, subckt, silent_risk
* bench.note: Silent twin of cap_lang_param_inside_subckt, whose resistor consumer makes RustSpice refuse loudly. ngspice-47 (tight) and an exact rational solve agree: v(out) = 3 V, v(n) = 3 V * 2k/(1k+2k) = 2 V. Minimal flip: the same .param card at top level is read correctly by RustSpice (3 V, 2 V). Slug silent-ignore-directive: a supported dot card is accepted and ignored in the subcircuit scope and the undefined reference defaults silently. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
.subckt SRC out
.param vv=3
V1 out 0 DC {vv}
.ends SRC
X1 out SRC
R1 out n 1k
R2 n 0 2k
.op
.end
