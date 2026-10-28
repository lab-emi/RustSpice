Distortion analysis request
* bench.id: cap_lang_ana_disto
* bench.group: capability
* bench.title: .disto dec 10 1k 10meg must be refused
* bench.probes: v(out)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-analysis
* bench.gap.note: .disto is replaced by a plain DC .op (exit 0, v(out) printed)
* bench.tags: nonlinear, disto, unsupported_analysis
* bench.note: ngspice-47 computes the 2nd and 3rd harmonic distortion plots (41 complex points each, not compared by the suite), so the checkable contract is the refusal; RustSpice prints nothing on stderr (checked). V1 carries DISTOF1 1: without a distortion input magnitude ngspice's two harmonic plots have every signal column identically zero (41 points, checked), i.e. a vacuous reference; with 'AC 1 DISTOF1 1' ngspice gives non-zero distortion (max |v(out)| 0.306 for the 2nd and 0.547 for the 3rd harmonic) while RustSpice is unchanged
V1 in 0 DC 0.7 AC 1 DISTOF1 1
R1 in d 1k
D1 d 0 DM
R2 d out 10k
C1 out 0 1n
.model DM D(IS=1e-14)
.disto dec 10 1k 10meg
.end
