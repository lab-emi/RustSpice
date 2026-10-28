negative inductance (-1 uH) with 1 kohm: growth rate exactly 1/tstep, 20 ns
* bench.id: rob_hs_l_negative_growth
* bench.group: robustness
* bench.title: negative inductance (-1 uH) with 1 kohm: growth rate exactly 1/tstep, 20 ns
* bench.probes: v(2)
* bench.expect: match
* bench.known_gap: fixed-step-accuracy
* bench.gap.note: the exponentially growing mode with lambda*h = 1 hits the backward-Euler singularity of the first step: v(2) = -1.05e25 V at 20 ns instead of +3.07e8 V; tstep 0.5n gives +6.3e8, 0.25n +3.6e8 (right sign, slow convergence)
* bench.tags: hostile, value, l_negative, tran, silent_risk
* bench.est_rss_mib: 64
* bench.note: exact (ODE with the 1 ns ramp): v2 grows as e^(t/1ns), v(2)(20 ns) = (e-1)*e^19 = 3.0668e8 V; ngspice with reltol=1e-9 gives 3.0671e8 (1e-4 relative); the 0.30 that the harness reports for ngspice as-is is a resampling artifact (linear interpolation of an exponentially growing waveform between its rows), not an ngspice error, while RustSpice returns about -1e25 V silently with exit 0 (tstep 0.1n gives 0.026 of swing, 2n gives 6.4e137)
V1 1 0 PULSE(0 1 0 1n 1n 1 2)
R1 1 2 1k
L1 2 0 -1u
.tran 1n 20n
.end
