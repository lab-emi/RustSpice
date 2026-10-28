Diode forward bias up to 1 A, current-driven (no RS)
* bench.id: cap_dev_dio_fwd_1a
* bench.group: capability
* bench.title: Diode forward bias up to 1 A, current-driven (no RS)
* bench.probes: v(a)
* bench.tol.rel: 1e-5
* bench.expect: match
* bench.tags: diode, dc, high_current
* bench.note: rel 1e-5: RustSpice stops Newton at reltol 1e-6 on the steep forward exponential (measured error 7e-7 V)
I1 0 a 1m
D1 a 0 DM
.model DM D(IS=1e-14)
.dc I1 1m 1 0.05
.end
