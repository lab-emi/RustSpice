Femtosecond-scale series RLC
* bench.id: acc_tr_tiny_lc
* bench.group: accuracy
* bench.title: Series RLC with L = 1 pH, C = 1 fF (f0 = 5 THz), R = 6.3 ohm (Q = 5), tstep 4 fs: time and component scales 6 decades below the usual
* bench.probes: v(out), i(v1)
* bench.tol.norm: 1e-2
* bench.oracle: tran_lti
* bench.tags: linear, tran, rlc, tiny_values
* bench.est_rss_mib: 16
* bench.note: Femtofarad charges are below ngspice default chgtol (1e-14 C), so the converged ngspice reference is only good to ~7e-4 here; the oracle is the truth.
V1 in 0 PULSE(0 1 0 1e-18 1e-18 1 2)
R1 in a 6.3246
L1 a out 1p
C1 out 0 1f
.tran 4f 2p
.end
