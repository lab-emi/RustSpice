RC response to an off-grid PULSE at tstep 0.2 ns
* bench.id: acc_tr_rc_pulse_offgrid_h0p2n
* bench.group: accuracy
* bench.title: RC (tau = 1 ns) driven by the acc_tr_rc_pulse_offgrid PULSE (td/tr/tf/pw off the grid) at tstep 0.2 ns, the tolerance-boundary member
* bench.probes: v(out)
* bench.tol.norm: 1e-2
* bench.expect: either
* bench.oracle: tran_lti
* bench.tags: linear, tran, timing_edge, ambiguous
* bench.est_rss_mib: 16
* bench.note: Ambiguous, so information only: RustSpice's error is 1.03e-2 of the swing against tol.norm 1e-2 (3 % over) while ngspice as-is is at 4.7e-3 and the converged reference and the tran_lti oracle agree, so the numbers are firm but the verdict is not: a 3 % change of the tolerance flips it, and the edge-sampling mechanism of the acc_tr_rc_pulse_offgrid gap (the PULSE edges at 0.37/0.9/3.2/3.61 ns fall between grid points, first-order convergence: 0.31, 0.22, 0.072 at 2, 1, 0.5 ns, 2.0e-3 at 0.1 ns) is already carried by the clearly failing members of that series. This member was taken out of the series because a series shares one known_gap; the tolerance is not loosened.
V1 in 0 PULSE(0 1 0.37n 0.53n 0.41n 2.3n 7.1n)
R1 in out 1k
C1 out 0 1p
.tran 0.2n 20n
.end
