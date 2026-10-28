negative capacitance (-1 nF) charged through 1 kohm: exponential growth, 100 ns
* bench.id: rob_hs_c_negative_growth
* bench.group: robustness
* bench.title: negative capacitance (-1 nF) charged through 1 kohm: exponential growth, 100 ns
* bench.probes: v(2)
* bench.expect: match
* bench.tags: hostile, value, c_negative, tran
* bench.est_rss_mib: 64
* bench.note: exact solution of C dv/dt = (v1-v)/R with the 1 ns pulse ramp (scipy ODE, rtol 1e-13): v(2)(100 ns) = -0.1046185168 V; the ideal-step closed form 1-exp(t/1us) = -0.1051709 V ignores the edge and is NOT the reference. ngspice with reltol=1e-9 and tmax 1p..0.1n is within 5e-7 of the exact value (1e-9 at 10 ps), so the converged reference is trustworthy and the old loosened tol.norm 1.5e-2 was wrong (it hid a 5.3e-3 RustSpice error)
V1 1 0 PULSE(0 1 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 -1n
.tran 2n 100n
.end
