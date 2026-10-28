
V1 in 0 DC 8
* bench.id: cap_lang_syn_blank_first_line
* bench.group: capability
* bench.title: an empty first line is the title; the card on line 2 (V1) is still a circuit element
* bench.probes: v(out)
* bench.tags: linear, op, syntax, title, silent_risk
* bench.note: ngspice-47 always takes physical line 1 as the title, even when it is empty (v(out) = 8 V * 1k/(3k+1k) = 2 V, hand-checked); V1 must be the first non-blank line, a comment line there would hide the defect; cap_lang_syn_no_title_line pins the agreeing case where line 1 holds a card Same title selection as rob_hs_ff_title_line_dot_card (RustSpice takes the first non-blank line that does not start with '.', '*' or ';' as the title; a whitespace-only first line fails the same way, verified): one root cause under two slugs, kept apart because here the card is lost silently (exit 0) while there the deck is refused. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
R1 in out 3k
R2 out 0 1k
.op
.end
