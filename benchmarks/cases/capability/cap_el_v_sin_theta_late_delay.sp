SIN with an off-grid delay, strong damping and offset: held at vo before td
* bench.id: cap_el_v_sin_theta_late_delay
* bench.group: capability
* bench.title: SIN with an off-grid delay, strong damping and offset: held at vo before td
* bench.probes: v(a), v(b)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref.tmax: 0.02n
* bench.tags: source, tran, waveform, sin, damping, delay
* bench.note: V1 = 1 + 2*exp(-(t-td)*2meg)*sin(2*pi*2.5meg*(t-td)) with td = 0.37u (not on the 10 ns grid): constant 1 V before td, then a decaying ring (envelope down to 2*exp(-6) at 3 us). V2 has theta 0 and delay 1.83u, so it stays at its offset for the first 1.83 us and then runs undamped. The source nodes are probed directly.
V1 a 0 SIN(1 2 2.5meg 0.37u 2meg)
R1 a 0 1k
V2 b 0 SIN(-0.5 1.5 1meg 1.83u)
R2 b 0 1k
.tran 10n 3u
.end
