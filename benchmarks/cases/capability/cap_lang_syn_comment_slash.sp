Double-slash comments
* bench.id: cap_lang_syn_comment_slash
* bench.group: capability
* bench.title: '//' full-line and trailing comments on a 3k:1k divider
* bench.probes: v(out)
* bench.expect: either
* bench.tags: linear, op, syntax, comment
* bench.note: Neither tool can be judged: '//' is a lexical extension of ngspice-47, not Berkeley SPICE syntax (classic comments are '*', ';' and '$'), so there is no independent oracle that '//' must be a comment. ngspice-47 accepts it (full line and trailing; v(out) = 8 V * 1k/(3k+1k) = 2 V, hand-checked); RustSpice refuses the full-line '//' loudly (exit 1, "Unsupported device card `//`"), a defensible choice. The trailing form 'R1 in out 3k // note' already runs in RustSpice (v(out) = 2 V) only because extra tokens on a card are ignored. Not verified: HSPICE, Xyce and LTspice (not installed).
V1 in 0 DC 8
// full line slash comment
R1 in out 3k // trailing note
R2 out 0 1k
.op
.end
