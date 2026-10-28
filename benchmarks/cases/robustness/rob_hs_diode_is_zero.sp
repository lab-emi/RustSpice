diode model with IS=0
* bench.id: rob_hs_diode_is_zero
* bench.group: robustness
* bench.title: diode model with IS=0
* bench.probes: v(2)
* bench.expect: either
* bench.tags: hostile, nonlinear, diode, model
* bench.est_rss_mib: 64
* bench.note: IS=0 means no diode current: RustSpice returns the mathematically exact v(2) = 4.999999995 V (5 V minus 1k*gmin*5 V). ngspice returns 1.5211501 V and the same value for IS=0, 1e-28, 1e-30 and 1e-35, i.e. it silently raises IS to a floor of 1e-28 A (IS=1e-25 gives 1.344 V). A model convention, not an error: ambiguous
V1 1 0 5
R1 1 2 1k
D1 2 0 DM
.model DM D(IS=0)
.op
.end
