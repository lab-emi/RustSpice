Circuit without any analysis card
* bench.id: cap_lang_syn_no_analysis_card
* bench.group: capability
* bench.title: a valid divider with no .op/.dc/.tran: the simulator must not invent an analysis silently
* bench.probes: v(out)
* bench.expect: either
* bench.tags: linear, syntax, invalid_input, ngspice_no_output
* bench.note: genuinely ambiguous, no verdict is possible: ngspice-47 runs nothing and writes no data when no analysis card is present (no rawfile, exit 0), so there is no reference number; RustSpice defaults to a DC .op (v(out) = 2 V, exit 0) as a deliberate convenience of its front end (an empty analysis list is replaced by .op in the netlist parser), which is neither provably wrong nor a case a user would ask the other tool for
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.end
