5 V to 12.5 V boost converter
* bench.id: acc_tr_boost_5v
* bench.group: accuracy
* bench.title: Boost converter: 5 V in, 10 uH, NMOS switch at 200 kHz / 60 % duty, diode, 10 uF / 100 ohm; output climbs from 4.4 V to ~10 V in 60 us
* bench.probes: v(out), v(sw)
* bench.tol.norm: 2e-2
* bench.tags: nonlinear, tran, mos, diode, power, switching
* bench.est_rss_mib: 16
* bench.note: Amp-level inductor/switch currents (peak ~1.5 A). The 48.9 us failure is the clamp, not the residual criterion: scaling every current down 2x, 5x, 10x and 100x (L, R, W, C scaled to keep the time constants) fails at the same instant 48.9 us, the failure time is the same for steps 20 ns, 40 ns and 100 ns, and a weaker switch (W = 200u instead of 2000u, v(sw) then stays below 10 V) completes. Unrelated dummy DC rails are no clean clamp test because the layers below the clamp then decide: with the frozen binary 6 V, 15 V, 40 V and 100 V rails complete (harness norm error 0.48-0.74 from the off-grid retry rows; at RustSpice's own sample times v(sw) of the 40 V run is 6.4e-3 of the swing from tight ngspice and v(out) 1.6e-3) while 8 V, 10 V, 12 V and 300 V rails fail. Reference verified: tight ngspice v(out) agrees with an independent Radau integration to 5.9e-6 of the swing (v(sw) is not compared: the 1 ns edges make the sample alignment dominate).
V1 in 0 DC 5
L1 in sw 10u
M1 sw g 0 0 NM W=2000u L=1u
Vg g 0 PULSE(0 5 0 1n 1n 3u 5u)
D1 sw out DM
C1 out 0 10u
R1 out 0 100
.model DM D(IS=1e-14 N=1.0)
.model NM NMOS(LEVEL=1 VTO=1.0 KP=200u LAMBDA=0.01)
.tran 20n 60u
.end
