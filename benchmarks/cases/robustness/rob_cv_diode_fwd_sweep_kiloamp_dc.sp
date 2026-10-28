Forward diode swept 0..5 V through 1 mohm (up to ~4 kA)
* bench.id: rob_cv_diode_fwd_sweep_kiloamp_dc
* bench.group: robustness
* bench.title: Forward diode swept 0..5 V through 1 mohm (up to ~4 kA)
* bench.probes: v(2)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, diode, large_current, dc
* bench.note: ngspice-47 completes the sweep; the failure is at a high-current point, not a topology problem. Title says up to ~4 kA at 5 V; 1 mohm in series. Calibration evidence: same mechanism as rob_cv_diode_fwd_kiloamp_op (the KCL residual of a converged point is rounding noise of terms gd*v of 1e5 A, above the absolute 1e-12 A criterion; the single point V1=5 V with 1 mOhm happens to converge, so the failure is rounding luck, not a clean current threshold). Reference verified: tight ngspice agrees with an independent 50-digit mpmath solve (Vt = 0.025864917007155805, gmin = 1e-12) at all 51 points to 3.5e-10 relative (v(2)); ngspice as-is is 2.6e-4 off at worst.
V1 1 0 0
R1 1 2 1m
D1 2 0 dm
.model dm D(is=1e-14 n=1)
.dc V1 0 5 0.1
.end
