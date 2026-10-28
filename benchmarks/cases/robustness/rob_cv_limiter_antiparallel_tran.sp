Anti-parallel diode limiter: 10 V / 1 kHz sine through 1k into two diodes
* bench.id: rob_cv_limiter_antiparallel_tran
* bench.group: robustness
* bench.title: Anti-parallel diode limiter: 10 V / 1 kHz sine through 1k into two diodes
* bench.probes: v(out)
* bench.tol.norm: 1e-3
* bench.expect: match
* bench.tags: nonlinear, diode, limiter, tran
* bench.note: Memoryless nonlinear network, so the exact reference is the algebraic root of (vin-v)/1k = Id(v)-Id(-v) at every time point (Brent, f64, gmin 1e-12 included). Calibration evidence: RustSpice matches this root to 7e-11 of the 1.43 V swing; ngspice as-is is 2.4e-2 off it (coarse steps across the diode knee and interpolation), tight ngspice (reltol 1e-6, 0.5u max step) 2.5e-4, which is why tol.norm is 1e-3 and not tighter. The waveform clips at +-0.71 V, so v(out) is far above the floor over most of the run.
V1 in 0 SIN(0 10 1k)
R1 in out 1k
D1 out 0 dm
D2 0 out dm
.model dm D(is=1e-14 n=1)
.tran 5u 3m
.end
