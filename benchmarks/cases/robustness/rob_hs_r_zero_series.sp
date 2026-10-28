R1 = 0 ohm in series with a 1 kohm load
* bench.id: rob_hs_r_zero_series
* bench.group: robustness
* bench.title: R1 = 0 ohm in series with a 1 kohm load
* bench.probes: v(1), v(2), i(v1)
* bench.tol.rel: 5e-2
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.tags: hostile, value, r_zero, dc, silent_risk
* bench.est_rss_mib: 64
* bench.note: ngspice warns "Value of resistor r1 is too small, set to 1e-12" and returns -4.8828125 mA, 2.3% off the exact -5 mA: an artefact of its own 1e-12 ohm floor (RustSpice with an explicit R1=1e-12 gives the same number), hence tol.rel 5e-2
V1 1 0 5
R1 1 2 0
R2 2 0 1k
.op
.end
