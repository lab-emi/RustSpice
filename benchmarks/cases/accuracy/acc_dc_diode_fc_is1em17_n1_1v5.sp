Diode IS=1e-17 N=1 from 1.5 V through 1k: boundary of the failure region in IS
* bench.id: acc_dc_diode_fc_is1em17_n1_1v5
* bench.group: accuracy
* bench.title: Diode IS=1e-17 N=1 from 1.5 V through 1k: boundary of the failure region in IS
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-12
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, small_is, silent_risk
* bench.note: oracle and tight ngspice agree to 6e-14 relative; mpmath (50 digits, gmin 1e-12, kT/q = 0.0258649170071575 V at 300.15 K) gives 0.8236712637 V, equal to tight ngspice to 3e-16 relative. Mechanism (second-look verification, private instrumented build, not re-run with the frozen binary): Newton declares convergence at iteration 2 because the KCL residual of the limited linearisation is 3.6e-14 A, below abstol 1e-12 A, while the diode limiter has moved the junction voltage; requiring the limiter to be inactive before declaring convergence closes all 10 false-convergence decks of this cluster within their own tolerances.
V1 in 0 1.5
R1 in d 1k
D1 d 0 dm
.model dm D(IS=1e-17 N=1)
.op
.end
