RC response to a PULSE whose edges are not on the step grid
* bench.id: acc_tr_rc_pulse_offgrid
* bench.group: accuracy
* bench.title: RC (tau = 1 ns) driven by a PULSE with td/tr/tf/pw off the 1 ns step grid
* bench.probes: v(out)
* bench.tol.norm: 1e-2
* bench.oracle: tran_lti
* bench.series.tstep: 2n 1n 0.5n 0.1n 0.05n
* bench.tags: linear, tran, timing_edge, series
* bench.est_rss_mib: 16
* bench.note: Series gap: the 2, 1 and 0.5 ns members confirm it, the 0.1 and 0.05 ns members pass. The 0.2 ns member (RustSpice 1.03e-2 against tol.norm 1e-2, 3 % over; ngspice as-is 4.7e-3) was judged ambiguous by the adversarial re-verification and moved out of the series into the information-only deck acc_tr_rc_pulse_offgrid_h0p2n: it sits on the tolerance boundary, so its verdict is set by the tolerance, not by the mechanism (a series shares one known_gap). Mechanism check (same circuit, 1 ns step): moving the corners onto the grid with PULSE(0 1 1n 1n 1n 2n 7n) cuts RustSpice's error from 0.22 to 0.045 (ngspice as-is 3.9e-3); the tran_lti oracle and the converged ngspice agree on every member.
V1 in 0 PULSE(0 1 0.37n 0.53n 0.41n 2.3n 7.1n)
R1 in out 1k
C1 out 0 1p
.tran 1n 20n
.end
