RC driven by a sine with a phase argument
* bench.id: acc_tr_rc_sin_phase
* bench.group: accuracy
* bench.title: RC low-pass (tau = 1 us) driven by SIN(0 1 100k 0 0 60): the phase argument (60 degrees) must be honoured, 100 steps per period
* bench.probes: v(out), v(in)
* bench.tol.norm: 5e-3
* bench.known_gap: silent-ignore-source-waveform
* bench.gap.note: The SIN phase argument (6th) is ignored: RustSpice evaluates the sine from phase 0 instead of 60 degrees, error 0.50 of swing on v(in) and 0.48 on v(out) (v(out) starts at 0 V instead of 0.866 V); no warning
* bench.tags: linear, tran, sin, silent_risk
* bench.est_rss_mib: 16
* bench.note: No oracle: SIN phase is outside tran_lti. Independent truth: v(in) = sin(2 pi f t + 60 deg) (tight ngspice within 1.8e-15 of swing at its own rows) and v(out) = the RC ODE solved from v(0) = sin(60 deg) = 0.866 V, the t = 0 operating point (tight ngspice within 2.9e-7 of swing at its own rows; as-is ngspice 2.4e-4 against the harness reference). A phase sweep 0/30/60/90 degrees leaves the RustSpice output bit-identical while ngspice moves; with phase 0 they agree (v(in) 5.6e-7, v(out) 1.6e-3 of swing, below tol.norm 5e-3)
V1 in 0 SIN(0 1 100k 0 0 60)
R1 in out 1k
C1 out 0 1n
.tran 100n 30u
.end
