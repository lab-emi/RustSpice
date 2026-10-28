Diode clamper (DC restorer): output swings from -9.3 V to +0.77 V
* bench.id: rob_cv_clamper_dc_restorer_tran
* bench.group: robustness
* bench.title: Diode clamper (DC restorer): output swings from -9.3 V to +0.77 V
* bench.probes: v(out)
* bench.expect: match
* bench.tags: nonlinear, clamper, negative_rail, tran
* bench.note: Regression anchor: a transient node 9 V below the source range works (the clamp depends on the instantaneous source value).
V1 in 0 SIN(0 5 1k)
C1 in out 10u
D1 out 0 dm
R1 out 0 100k
.model dm D(is=1e-14 n=1)
.tran 5u 5m
.end
