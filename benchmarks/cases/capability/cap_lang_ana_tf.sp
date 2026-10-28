Transfer function request
* bench.id: cap_lang_ana_tf
* bench.group: capability
* bench.title: .tf v(out) V1 (small-signal gain, input and output resistance) must be refused
* bench.probes: v(out)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-analysis
* bench.gap.note: .tf is replaced by a plain DC .op (exit 0, v(out) printed)
* bench.tags: linear, tf, unsupported_analysis
* bench.note: ngspice-47 computes the transfer function (gain 0.75, input resistance 4 kohm, output resistance 750 ohm, hand-checked) but the suite compares only op/dc/tran plots, so the observable contract is the refusal; a future RustSpice .tf would keep this case at known_gap_confirmed until it is retargeted to a numeric comparison; RustSpice prints nothing on stderr (checked). Caveat on the loud_error expectation (SUITE_SPEC 13.1 reserves it for genuinely out-of-model requests): .tf is a DC-domain analysis that RustSpice could implement, so the case can only leave known_gap_confirmed by refusing, never by implementing the feature, until the engine can compare scalar transfer-function plots (expect match)
V1 in 0 DC 1
R1 in out 1k
R2 out 0 3k
.tf v(out) V1
.end
