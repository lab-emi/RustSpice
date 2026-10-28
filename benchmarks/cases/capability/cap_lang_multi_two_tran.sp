Two transient cards
* bench.id: cap_lang_multi_two_tran
* bench.group: capability
* bench.title: .tran 10n 2u followed by .tran 10n 5u: two transient plots, one per card
* bench.probes: v(out)
* bench.expect: either
* bench.tags: linear, multi_analysis, tran
* bench.note: genuinely ambiguous, no verdict is possible: RustSpice writes the plots in card order (2 us run, then 5 us run) but ngspice-47 writes them in reverse card order (5 us plot first, 2 us plot last; swapping the two cards swaps the plots, checked), and the suite compares the last plot of each kind, i.e. RustSpice's 5 us plot against ngspice's 2 us plot over the common window [0, 2 us] only, where they agree; nothing says which plot order is right and the comparison cannot tell whether both runs were executed
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 10n 2u
.tran 10n 5u
.end
