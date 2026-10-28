two resistors both named R1
* bench.id: rob_hs_dup_element_name
* bench.group: robustness
* bench.title: two resistors both named R1
* bench.probes: v(1), v(2)
* bench.expect: loud_error
* bench.tags: hostile, duplicate, parser, silent_risk
* bench.est_rss_mib: 64
* bench.note: duplicate element names are invalid in every SPICE dialect (the instance name is the unique key, e.g. for i(r1) or .dc R1); refusing is the correct behaviour. Verified with ngspice-47 (no solution produced) and RustSpice 7bb6d36 (exit 0, v(2)=3.75 V = 5*3k/4k, i.e. both resistors present). There is no unique truth: first card wins gives v(2)=5.0 V (R1 1 2 1k to an open node), last card wins gives 0.0 V; renaming the second resistor to R2 makes both tools agree (v(2)=3.75 V, i(v1)=-1.25e-3 A). Same defect as cap_lang_syn_duplicate_element [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 5
R1 1 2 1k
R1 2 0 3k
.op
.end
