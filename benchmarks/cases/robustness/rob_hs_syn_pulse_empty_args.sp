PULSE() without arguments
* bench.id: rob_hs_syn_pulse_empty_args
* bench.group: robustness
* bench.title: PULSE() without arguments
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser, source, silent_risk
* bench.est_rss_mib: 64
* bench.note: Argument validation, not parsing: a valid PULSE is read correctly by RustSpice, so the gap closes only when too few PULSE arguments are refused [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
V9 3 0 PULSE()
R9 3 0 1k
.op
.end
