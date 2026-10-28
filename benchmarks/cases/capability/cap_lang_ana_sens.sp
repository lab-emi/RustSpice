Sensitivity analysis request
* bench.id: cap_lang_ana_sens
* bench.group: capability
* bench.title: .sens v(out) (DC sensitivities) must be refused by the netlist front end
* bench.probes: v(out)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-analysis
* bench.gap.note: .sens is replaced by a plain DC .op (exit 0, v(out) printed)
* bench.tags: linear, sens, unsupported_analysis
* bench.note: ngspice-47 computes the sensitivities (dv(out)/dR1 = -1.875e-4, dv(out)/dR2 = 6.25e-5 V/ohm, dv(out)/dV1 = 0.75, hand-checked) but the suite compares only op/dc/tran plots, so the observable contract is the refusal; a future RustSpice .sens would keep this case at known_gap_confirmed until it is retargeted to a numeric comparison; RustSpice prints nothing on stderr (checked). Caveat on the loud_error expectation (SUITE_SPEC 13.1 reserves it for genuinely out-of-model requests): .sens is a DC-domain analysis that RustSpice could implement (its adjoint subcommand already computes DC sensitivities), so the case can only leave known_gap_confirmed by refusing, never by implementing the feature, until the engine can compare scalar sensitivity plots (expect match)
V1 in 0 DC 1
R1 in out 1k
R2 out 0 3k
.sens v(out)
.end
