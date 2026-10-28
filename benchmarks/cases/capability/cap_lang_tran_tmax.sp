Transient with a maximum internal step
* bench.id: cap_lang_tran_tmax
* bench.group: capability
* bench.title: 4-argument .tran 1u 3m 0 0.5u (tstart = 0, tmax = 0.5 us) on a 1 kHz sine through an RC
* bench.probes: v(out)
* bench.tags: linear, tran, tmax
* bench.note: tmax only bounds the internal step; the step (1 us) is already fine enough for a fixed-step solver, so only parsing of the 4th argument is under test
V1 in 0 SIN(0 1 1k)
R1 in out 1k
C1 out 0 100n
.tran 1u 3m 0 0.5u
.end
