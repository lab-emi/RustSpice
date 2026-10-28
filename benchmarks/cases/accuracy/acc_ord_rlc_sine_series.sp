Series RLC driven by a smooth sine (50 ohm, 100 nH, 10 pF, 100 MHz)
* bench.id: acc_ord_rlc_sine_series
* bench.group: accuracy
* bench.title: Series RLC (Q = 2, f0 = 159 MHz) driven by SIN(0 1 100meg): smooth drive without an edge, consistent zero start, 10 periods, 80 steps per period at the nominal step
* bench.probes: v(out), i(v1)
* bench.tol.norm: 3e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.series.tstep: 1n 500p 250p 125p 62.5p 31.25p
* bench.tags: linear, tran, rlc, sin, series, order
* bench.est_rss_mib: 16
* bench.note: Order-of-convergence deck. The existing RLC series acc_tr_rlc_q5 is driven by a PULSE edge that lies inside the first step; this one by a smooth sine from a consistent zero state, so the global error is pure trapezoid truncation. Truth is the closed-form phasor plus homogeneous solution (numpy, complex eigen-decomposition), cross-checked with scipy DOP853 rtol 1e-12 to 8e-12 of the swing. RustSpice error vs truth 1.49e-1/5.0e-2/1.36e-2/3.46e-3/8.66e-4/2.16e-4 V for h = 1n..31.25p (v(out) pair slopes 1.97/2.00/2.00; i(v1) 1.73/1.89/1.96, approaching 2 from below because of the BDF-1 start-up step). The suite reference run (reltol 1e-6, tmax = h/20) is within 5.7e-6 V of the truth at 125p; ngspice with reltol=1e-9 and tmax=h floors flat at 9.6e-6 V. tol.norm 3e-3 is 1.4x RustSpice's own trapezoid error at the nominal 125 ps, not a reference limit; coarser members are info. Gates: checks/tran_order_and_conservation_oracles/check_order_convergence.py (order, reference bound), checks/tran_order_and_conservation_oracles/check_conservation.py (energy balance).
V1 in 0 SIN(0 1 100meg)
R1 in a 50
L1 a out 100n
C1 out 0 10p
.tran 125p 100n
.end
