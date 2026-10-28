DC sweep followed by a transient
* bench.id: cap_lang_multi_dc_then_tran
* bench.group: capability
* bench.title: .dc of a pulse source then .tran: the transient must still see the PULSE waveform
* bench.probes: v(out)
* bench.tags: linear, multi_analysis, tran, dc
* bench.note: Independent truth: scipy solution of the RC response to PULSE(0 5 1u 1n 1n 4u 10u): tight ngspice is within 7.0e-7 of swing at its own rows. Caveat: RustSpice's .tran alone (no .dc, or .tran placed before the .dc) differs from ngspice by 0.043 of swing on this deck because the 1 ns PULSE edges fall inside the 10 ns steps (fixed-step edge error), so fixing the restore alone would not close this deck; cap_lang_multi_dc_then_tran_slow_edges isolates the clobber with edges the grid resolves
V1 in 0 PULSE(0 5 1u 1n 1n 4u 10u)
R1 in out 1k
C1 out 0 100p
.dc V1 0 5 1
.tran 10n 6u
.end
