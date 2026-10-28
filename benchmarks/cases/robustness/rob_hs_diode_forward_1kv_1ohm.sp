case: 1 kV through 1 ohm into a diode (999 A forward current)
* bench.id: rob_hs_diode_forward_1kv_1ohm
* bench.group: robustness
* bench.title: 1 kV through 1 ohm into a diode (999 A forward current)
* bench.probes: v(2), i(v1)
* bench.tol.rel: 1e-5
* bench.tol.abs: 1e-6
* bench.oracle: dc_nonlinear
* bench.expect: match
* bench.tags: hostile, nonlinear, overflow, diode, abstol
* bench.est_rss_mib: 64
* bench.note: Same circuit as acc_dc_diode_fwd_kiloamp (different tolerances only). Reference verified: an independent 50-digit mpmath solve of the Shockley model (Vt = 0.025864917007155805, gmin = 1e-12) gives v(2) = 1.01242873 V, i = 998.98757127 A; tight ngspice agrees to 6.4e-14 (v) and 9e-17 (i), ngspice as-is is 1.6e-7 and 1.7e-10 off (inside tol.rel 1e-5 / tol.abs 1e-6, tolerances unchanged).
V1 1 0 1000
R1 1 2 1
D1 2 0 DM
.model DM D(IS=1e-14)
.op
.end
