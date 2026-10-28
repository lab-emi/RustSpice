Two elements with the same name
* bench.id: cap_lang_syn_duplicate_element
* bench.group: capability
* bench.title: two cards named R1 (3k and 1k) in one circuit
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, op, syntax, invalid_input, element_names, ngspice_error_ok
* bench.note: ngspice-47 refuses the deck ("device already exists, bail out", exit 1, no rawfile), also for a case-insensitive duplicate (R1 and r1). There is no unique truth, which is why refusing is the only correct answer: both resistors present gives 2.0 V, first card wins gives 8.0 V (3k to an open node), last card wins gives 0.0 V (1k to ground only). Renaming one resistor makes both tools agree (v(out) = 2.0 V). Same defect as rob_hs_dup_element_name; with duplicate V sources RustSpice's i(v1) and a `.dc V1` sweep silently refer to only one of the two [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 DC 8
R1 in out 3k
R1 out 0 1k
.op
.end
