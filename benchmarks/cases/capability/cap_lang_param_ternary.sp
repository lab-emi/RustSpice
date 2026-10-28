Ternary operator in a brace expression
* bench.id: cap_lang_param_ternary
* bench.group: capability
* bench.title: R1 {sel > 1 ? 3k : 1k} takes 3k for sel = 2
* bench.probes: v(out)
* bench.tags: linear, op, param, expression, silent_risk
* bench.note: ngspice-47 evaluates the conditional (v(out) = 8 V * 1k/(3k+1k) = 2 V, hand-checked) Same root as cap_lang_param_brace_spaces (blank-splitting of the brace token), silent here only because the first piece `{sel` names a defined parameter; the expr-truncation slug fits loosely: `{sel>1?3k:1k}` (no blanks) is refused loudly by RustSpice where ngspice gives the same 2 V (verified). [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
.param sel=2
V1 in 0 DC 8
R1 in out {sel > 1 ? 3k : 1k}
R2 out 0 1k
.op
.end
