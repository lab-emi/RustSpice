current source into a node with no other element
* bench.id: rob_hs_sing_current_source_dead_end
* bench.group: robustness
* bench.title: current source into a node with no other element
* bench.probes: v(1), v(2)
* bench.expect: either
* bench.tags: hostile, singular, dc, floating, ill_posed, current_cutset, silent_risk
* bench.est_rss_mib: 64
* bench.note: KCL at node 1 reads 1 mA = 0: no DC solution exists, so no value is correct. Same circuit as rob_cv_sing_isrc_cutset_op (duplicate witness). Verified with both tools: ngspice-47 exits 0 and returns v(1) = 1 mV with no warning (tight options too), RustSpice exits 0 and returns v(1) = 1e9 V (=I/gmin) with no diagnostic; neither refuses or warns, so neither can be judged and expect: either (policy 13.1). The well-posed v(2)=1 V and i(v1) agree in both.
I1 0 1 1m
V1 2 0 1
R1 2 0 1k
.op
.end
