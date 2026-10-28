model card with an unknown device type
* bench.id: rob_hs_syn_model_unknown_type
* bench.group: robustness
* bench.title: model card with an unknown device type
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser, model, silent_risk
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
D9 2 0 DM
.model DM FROB(IS=1e-12)
.op
.end
