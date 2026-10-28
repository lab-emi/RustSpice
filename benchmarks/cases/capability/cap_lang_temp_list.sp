.temp card with several temperatures
* bench.id: cap_lang_temp_list
* bench.group: capability
* bench.title: .temp 0 27 85 lists three temperatures
* bench.probes: v(out)
* bench.expect: either
* bench.tags: nonlinear, op, temperature
* bench.note: genuinely ambiguous, no verdict is possible: ngspice-47 cannot run a temperature list (warning 'Could not set temperature to 0 27 85') and silently falls back to its default 27 C (v(out) = 0.6929 V, textbook 0.69289 V), RustSpice ignores .temp altogether and also runs at 27 C (300.15 K), so the numbers agree by coincidence and the deck cannot tell a working list from an ignored one; the checkable temperature defect is cap_lang_temp_single (.temp 85)
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DM
.model DM D(IS=1e-14)
.temp 0 27 85
.op
.end
