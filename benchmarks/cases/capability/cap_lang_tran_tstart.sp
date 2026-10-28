Transient with a start time
* bench.id: cap_lang_tran_tstart
* bench.group: capability
* bench.title: .tran 10n 5u 2u records only t >= 2 us of an RC step
* bench.probes: v(out)
* bench.tags: linear, tran, tstart
* bench.note: ngspice-47 emits rows only for t >= tstart (first row near 2 us); RustSpice ignores tstart and emits from t = 0 (a superset whose values agree on the overlap), so compare on [tstart, tstop]
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 10n 5u 2u
.end
