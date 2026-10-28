model card with an unknown device type
* bench.id: rob_hs_syn_model_unknown_type
* bench.group: robustness
* bench.title: model card with an unknown device type
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-directive
* bench.gap.note: a .model card of unknown type is accepted and ignored, the diode D9 runs with default parameters (v(2) = 0.6884 V, exit 0, empty stderr; with the type corrected to D, whose IS=1e-12 applies, RustSpice and ngspice give 0.5709 V, mpmath 0.5709282 V); ngspice-47 aborts ('model type mismatch', exit 1). A diode that references an undefined model name is silently defaulted the same way
* bench.tags: hostile, syntax, parser, model, silent_risk
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
D9 2 0 DM
.model DM FROB(IS=1e-12)
.op
.end
