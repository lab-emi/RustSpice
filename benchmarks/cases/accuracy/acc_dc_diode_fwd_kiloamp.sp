Diode IS=1e-14 fed from 1000 V through 1 ohm: 999 A through the junction
* bench.id: acc_dc_diode_fwd_kiloamp
* bench.group: accuracy
* bench.title: Diode IS=1e-14 fed from 1000 V through 1 ohm: 999 A through the junction
* bench.probes: v(d), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, large_signal
* bench.note: ngspice default (as-is) tolerances solve it. Reference verified: tight ngspice agrees with an independent 50-digit mpmath solve of the Shockley model (Vt = 0.025864917007155805, gmin = 1e-12; v(d) = 1.01242873 V, i = 998.98757127 A) to 6.4e-14 (v(d)) and 9e-17 (i); ngspice as-is is 1.6e-7 and 1.7e-10 off. Reproduced: 1000 V / 1 ohm and 1000 V / 0.1 ohm fail (residual stops at 1.8e-12 A = 2^-39, the rounding noise of the 1e3 A terms); the same 1 kA with 2000 V / 2 ohm or 50 V / 0.05 ohm converges, and the 1 ohm family converges at 22 of 29 supplies in 100..1500 V, so the criterion is a rounding lottery, not a smooth current threshold.
V1 in 0 1000
R1 in d 1
D1 d 0 dm
.model dm D(IS=1e-14 N=1)
.op
.end
