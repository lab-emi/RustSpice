VCCS driven by a 10 MHz sine into a diode-clamped RC load
* bench.id: cap_xm_g_tran_sine_diode_rc
* bench.group: capability
* bench.title: VCCS (2 mS) from a 10 MHz sine into 1k || 20 pF with a diode clamp, tstep 1 ns
* bench.probes: v(out)
* bench.tol.norm: 3e-3
* bench.expect: match
* bench.tags: controlled, vccs, tran, diode, sine, npoints
* bench.est_rss_mib: 16
* bench.note: G x .tran x diode (the corpus has 3 G .tran decks, none with a diode, and no G + diode transient at all). v(out) swings from -1.3087 V (diode reverse, RC lag) to +0.6626 V (clamped). Independent oracle (hand-derived one-node ODE C dv/dt = gm*vin - v/R - Id(v), scipy Radau rtol 1e-12, checks/element_analysis_matrix_holes/check_independent_oracles.py): converged ngspice reference 3.3e-6 of swing from it, RustSpice 1.47e-3 at the fixed 1 ns step (trapezoid, 100 steps per period; 2nd-order truncation error), ngspice as-is 1.24e-3 at its own steps (same size, 308 rows vs 301), so tol.norm 3e-3 is 2x the RustSpice error and below the 5e-3 default. Replacing IS by 1e-12 moves RustSpice to >10x tolerance.
V1 in 0 SIN(0 1 10meg)
G1 0 out in 0 2m
R1 out 0 1k
C1 out 0 20p
D1 out 0 dm
.model dm D(IS=1e-14 N=1)
.tran 1n 300n
.end
