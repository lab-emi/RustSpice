Transient with step larger than stop time
* bench.id: cap_lang_tran_step_gt_stop
* bench.group: capability
* bench.title: .tran 10u 1u (tstep > tstop) still integrates over [0, 1 us]; tau = 1 ms keeps a single step accurate
* bench.probes: v(out)
* bench.floor: 1e-4
* bench.tags: linear, tran
* bench.note: a harness grid built from tstep degenerates to t = 0 when tstep > tstop; compare on the union of both time axes
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1u
.tran 10u 1u
.end
