Parameter stepping request
* bench.id: cap_lang_ana_step
* bench.group: capability
* bench.title: .step param rv 1k 3k 1k (LTspice-style stepping) next to a valid .op
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, step, unsupported_analysis, ngspice_error_ok
* bench.note: ngspice-47 rejects .step (exit 1, 'circuit not parsed', no rawfile): LTspice syntax is not ngspice syntax, so both tools should refuse; RustSpice drops it and runs one .op with rv = 1k (v(out) = 0.5 V, raw output bit-identical to the deck without the .step card), exit 0, no message; it is the same netlist catch-all that swallows unknown dot-cards (cap_lang_dir_unknown_card, rob_hs_syn_unknown_dot_card, slug lenient-malformed-input there), kept under silent-ignore-analysis because .step is a named analysis-class card in the slug definition [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
.param rv=1k
V1 in 0 DC 1
R1 in out {rv}
R2 out 0 1k
.step param rv 1k 3k 1k
.op
.end
