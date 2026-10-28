Pulse narrower than the time step
* bench.id: acc_tr_pulse_narrow
* bench.group: accuracy
* bench.title: 5 V / 100 ps pulse into an RC (tau = 10 ns) sampled every 10 ns: the output peak (55 mV from the injected charge, 20 mV at the first grid sample after the pulse) must not be lost
* bench.probes: v(out)
* bench.tol.norm: 5e-2
* bench.known_gap: fixed-step-accuracy
* bench.gap.note: The 100 ps pulse falls between grid points and is never seen (PULSE is sampled only at grid points): v(out) stays identically 0 V while the true response peaks at 54.7 mV at 100.12 ns (20.4 mV at the 110 ns grid row); harness norm error 0.37 of that 54.7 mV swing (1.0 of the grid-sampled swing), ngspice as-is resolves the peak and is at 8.9e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, timing_edge, lost_pulse
* bench.est_rss_mib: 16
* bench.note: Closed form: the 100 ps (110 ps with the 10 ps edges) pulse into R = 10 ohm, C = 1 nF gives a peak of 54.66 mV at 100.12 ns and 20.35 mV at the 110 ns grid row; the converged ngspice reference and the tran_lti oracle agree (8.9e-3 and 8.8e-3 for ngspice as-is). Tolerance 5e-2 of the 54.7 mV swing is 2.7 mV, 5.6x ngspice's as-is error (0.49 mV), so a real fix passes. Flip tests (only tstep changed): 1 ns gives 0.92, 0.1 ns 0.41, 10 ps 7.6e-6 (the gap closes only once a step lands inside the pulse).
V1 in 0 PULSE(0 5 100n 10p 10p 100p 1u)
R1 in out 10
C1 out 0 1n
.tran 10n 1u
.end
