RC response to a PULSE with tr = 0
* bench.id: acc_tr_rc_pulse_zero_edge
* bench.group: accuracy
* bench.title: RC (tau = 1 ns) driven by a PULSE with tr = 0 (ideal step at 1 ns), 500 steps per tau
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.tags: linear, tran, timing_edge
* bench.est_rss_mib: 16
* bench.note: ngspice maps tr=0 to TSTEP; with tstep << tau the leading-edge convention (ideal step vs TSTEP ramp) shifts the response by tstep/2, i.e. < 1e-3 of the swing. Only the leading edge is exercised (the trailing-edge position also moves by tr under that convention). No oracle: tr = 0 is simulator-defined (ngspice maps it to TSTEP). Fragile pass: RustSpice reads the accumulated grid time 0.9999999999999999 ns (500 steps of 2 ps) as still below td, which makes its ideal step coincide with ngspice's TSTEP-ramp; with tstep 20 ps the same accumulation reaches td one step earlier and the two tools differ by a whole step (see acc_tr_rc_pulse_zero_edge_pw, expect=either).
V1 in 0 PULSE(0 1 1n 0 0 50n 100n)
R1 in out 1k
C1 out 0 1p
.tran 2p 5n
.end
