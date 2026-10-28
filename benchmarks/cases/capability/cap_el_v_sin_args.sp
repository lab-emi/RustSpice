SIN with offset, delay and damping (three sources)
* bench.id: cap_el_v_sin_args
* bench.group: capability
* bench.title: SIN with offset, delay and damping (three sources)
* bench.probes: v(a), v(b), v(c)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: source, tran, waveform, sin
* bench.note: V3 is the damped, delayed sine vo + va*exp(-(t-td)*theta)*sin(2*pi*f*(t-td)). Observed difference between simulators is below 5e-4 of the swing and comes from the ngspice reference (linear interpolation at the td corner); the tran_lti oracle puts RustSpice at 5e-10 of the closed form (ngspice at its own time points matches the closed form to 4e-15; resampled as-is it is at 8e-3). The check is not blunt: scaling a source by 5 % or moving td moves the probes by far more than the 2e-3 tolerance.
V1 a 0 SIN(2.5 1 1meg)
R1 a 0 1k
V2 b 0 SIN(0 1 1meg 1u)
R2 b 0 1k
V3 c 0 SIN(0 1 1meg 1u 500k)
R3 c 0 1k
.tran 10n 5u
.end
