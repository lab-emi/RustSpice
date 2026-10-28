Spaces inside the braces
* bench.id: cap_lang_param_brace_spaces
* bench.group: capability
* bench.title: R1 { ra } (blanks inside the braces) is the same as R1 {ra}
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: Invalid resistance value `{` (blanks inside the braces split the token)
* bench.tags: linear, op, param, syntax
* bench.note: ngspice-47 computes v(out) = 8 V * 1k/(3k+1k) = 2 V (hand-checked) Same mechanism as cap_lang_param_ternary: the brace expression is split at blanks and the braces are stripped per token; `{ra }` and `{ra  }` work by accident (v(out) = 2 V) while `{ ra}` and `{ ra }` are refused (verified).
.param ra=3k
V1 in 0 DC 8
R1 in out { ra }
R2 out 0 1k
.op
.end
