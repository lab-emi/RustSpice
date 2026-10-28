model parameter list missing its closing parenthesis
* bench.id: rob_hs_syn_model_unbalanced_paren
* bench.group: robustness
* bench.title: model parameter list missing its closing parenthesis
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: hostile, syntax, parser, model, lenient_ok
* bench.est_rss_mib: 64
* bench.note: ngspice-47 and RustSpice both accept the missing ')' and honour the parameter list. IS=1e-12 (not the default 1e-14) so that a silently dropped list would be visible: v(2)=0.5745 V with the model, 0.6929 V with defaults. Strengthened in calibration
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
D9 2 0 DM
.model DM D(IS=1e-12
.op
.end
