temp -300 (below absolute zero) on a diode circuit
* bench.id: rob_hs_temp_below_absolute_zero
* bench.group: robustness
* bench.title: .temp -300 (below absolute zero) on a diode circuit
* bench.probes: v(2)
* bench.expect: loud_error
* bench.tags: hostile, temperature, directive, silent_risk
* bench.est_rss_mib: 64
* bench.note: -300 C is below absolute zero (-273.15 C, i.e. -26.85 K), so no physical answer exists and refusing is the only correct outcome. ngspice-47's refusal is a convergence failure, not validation: it fails the same way at -273.15, -273 and -400 C but solves -250 C (v(2) about 1.09 V), so its failure is not evidence of intentional validation. RustSpice drops the card (identical 0.6929 V output for .temp -250, -300 and 100 alike), hence the slug; once .temp is implemented the residual defect would be missing T>0 K validation (lenient-malformed-input) [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
.temp -300
V1 1 0 5
R1 1 2 1k
D1 2 0 DM
.model DM D(IS=1e-14)
.op
.end
