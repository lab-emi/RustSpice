1 MA current source into a diode in parallel with 1 mohm (diode carries ~1e6 A, v=1.191 V)
* bench.id: rob_cv_scale_1ma_diode_mohm_op
* bench.group: robustness
* bench.title: 1 MA current source into a diode in parallel with 1 mohm (diode carries ~1e6 A, v=1.191 V)
* bench.probes: v(1)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.est_rss_mib: 32
* bench.tags: nonlinear, diode, large_current, op
* bench.note: ngspice-47 v(1)=1.19109262 V; my numpy Newton agrees. Without the 1 mohm resistor (diode alone, 1 MA) RustSpice converges and matches (1.191123447 V), so the resistor/diode current split is what breaks the criterion. Calibration evidence: independent dense f64 Newton (gmin->0) matches ngspice-tight to 7.6e-14 V, with its own KCL residual 5.8e-10 A (the largest term gd*v is 4.6e7 A, ulp 7.5e-9), so an f64 solver meets the 1e-12 A criterion at this scale only by rounding luck: the I = 0.5..2 MA family (31 currents) passes at 20 of 31 with the default criterion, at 22 of 31 with the node-voltage box disabled (this deck itself then passes by luck, so a clamp-only change closes it spuriously) and at 31 of 31 with abstol 1e-6 (this deck: abstol 1e-9 still fails, 1e-6 matches). Reference verified: tight ngspice agrees with an independent 50-digit mpmath solve (Vt = 0.025864917007155805, gmin = 1e-12; v(1) = 1.19109262077 V) to 6.4e-14; ngspice as-is is 1.4e-7 off.
I1 0 1 1meg
R1 1 0 1m
D1 1 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
