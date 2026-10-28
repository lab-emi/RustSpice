Buck converter
* bench.id: acc_tr_buck_converter
* bench.group: accuracy
* bench.title: 12 V to ~5 V buck: NMOS high-side switch (floating gate drive), freewheel diode, 47 uH / 22 uF / 5 ohm, 100 kHz 40 % duty
* bench.probes: v(out), v(sw)
* bench.tol.norm: 1.5e-2
* bench.ref: reltol=1e-6 abstol=1e-12 vntol=1e-6
* bench.ref.tmax: 2.5n
* bench.tags: nonlinear, tran, mos, diode, power, switching
* bench.est_rss_mib: 16
* bench.note: ngspice reports "Timestep too small" for the reference at abstol=1e-15 / vntol=1e-9; the reference uses abstol=1e-12 vntol=1e-6 with tmax=tstep/20 (agrees with tmax=tstep/80 to 3e-4 of the swing). Calibration: passes at 50 ns only by luck of the step grid; RustSpice fails loudly at 15 ns, 12.5 ns, 10 ns and 5 ns (see acc_tr_buck_converter_fine) and at 25 ns mis-samples v(sw) by 0.68 of swing after a step retry (output grid 3 ns off); at 50 ns v(sw) is within 2e-5 and v(out) within 8.3e-3 (ngspice as-is 6.9e-4).
VIN in 0 DC 12
M1 in g sw 0 NM W=500u L=1u
VG g sw PULSE(0 12 0 20n 20n 4u 10u)
D1 0 sw DM
L1 sw out 47u
C1 out 0 22u
RL out 0 5
.model NM NMOS(LEVEL=1 VTO=2 KP=20m LAMBDA=0.0)
.model DM D(IS=1e-10 N=1.0)
.tran 50n 100u
.end
