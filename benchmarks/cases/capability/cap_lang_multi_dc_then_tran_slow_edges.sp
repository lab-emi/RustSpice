DC sweep of a pulse source followed by a transient with resolvable edges
* bench.id: cap_lang_multi_dc_then_tran_slow_edges
* bench.group: capability
* bench.title: .dc of a pulse source then .tran with 100 ns PULSE edges: the transient must still see the PULSE waveform
* bench.probes: v(out)
* bench.expect: match
* bench.tags: linear, multi_analysis, tran, dc
* bench.est_rss_mib: 16
* bench.note: Closer twin of cap_lang_multi_dc_then_tran: the PULSE edges are 100 ns (ten 10 ns steps) so that RustSpice's .tran alone agrees with ngspice (3.1e-4 of swing, below the default tol.norm 5e-3) and the only defect left is the sweep overwriting the waveform. Independent truth: scipy DOP853 (rtol 1e-13) solution of the RC response (tau = 100 ns) to PULSE(0 5 1u 100n 100n 4u 10u): tight ngspice is within 2.8e-6 of swing and RustSpice .tran alone within 3.1e-4 (both after resampling to the 10 ns grid). The .dc block matches ngspice
V1 in 0 PULSE(0 5 1u 100n 100n 4u 10u)
R1 in out 1k
C1 out 0 100p
.dc V1 0 5 1
.tran 10n 6u
.end
