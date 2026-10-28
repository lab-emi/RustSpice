Element ic= and .ic on the same node
* bench.id: cap_lang_ic_element_and_dot_ic
* bench.group: capability
* bench.title: C1 ic=1 and .ic v(out)=5 disagree on the starting voltage: ngspice starts at 1 V (element wins), RustSpice at 5 V (.ic wins)
* bench.probes: v(out)
* bench.expect: either
* bench.tags: linear, tran, uic, ic
* bench.note: genuinely ambiguous, no verdict is possible: nothing says whether an element ic= or a .ic card has precedence. ngspice-47 starts at 1 V (first row 0.9999 V, v(5us) = exp(-5) = 6.74 mV, hand-checked for tau = 1 us), RustSpice reads the .ic card and ignores element ic= altogether; the defect that can be judged, element ic= with no .ic card, is pinned by cap_lang_ic_element_cap_inductor
R1 out 0 1k
C1 out 0 1n ic=1
.ic v(out)=5
.tran 10n 5u uic
.end
