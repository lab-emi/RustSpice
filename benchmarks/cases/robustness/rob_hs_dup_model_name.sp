same .model name defined twice with different IS
* bench.id: rob_hs_dup_model_name
* bench.group: robustness
* bench.title: same .model name defined twice with different IS
* bench.probes: v(1), v(2)
* bench.expect: either
* bench.tags: hostile, duplicate, parser, model
* bench.est_rss_mib: 64
* bench.note: undefined by the language: ngspice keeps the FIRST definition silently (v(2)=0.6929 V, IS=1e-14), RustSpice the LAST (0.5745 V, IS=1e-12); neither warns. No SPICE convention makes either right: ambiguous
V1 1 0 5
R1 1 2 1k
D1 2 0 DM
.model DM D(IS=1e-14)
.model DM D(IS=1e-12)
.op
.end
