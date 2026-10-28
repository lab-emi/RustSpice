MOS sample and hold
* bench.id: acc_tr_sample_hold
* bench.group: accuracy
* bench.title: NMOS sampling switch (clock 100 ns every 400 ns) onto a 2 pF hold capacitor, input 1 MHz sine around 1.6 V
* bench.probes: v(out)
* bench.tol.norm: 1e-2
* bench.tags: nonlinear, tran, mos, switched_capacitor
* bench.est_rss_mib: 16
* bench.note: tstep 0.25 ns (2 steps per 1 ns switch edge): RustSpice 5.3e-3, ngspice as-is 4.3e-3 of swing, both dominated by the sampling edges; RustSpice converges 2nd order (2.3e-2 at 0.5 ns, 1.3e-3 at 0.125 ns, 3.3e-4 at 0.0625 ns). Was 0.5 ns with tol 4e-2, which a 5 % change of the switch width or the hold capacitor could not violate (mutation check); at 1e-2 both are detected.
VDD vdd 0 DC 3.3
VIN in 0 SIN(1.6 0.8 1Meg)
VCK clk 0 PULSE(0 3.3 20n 1n 1n 100n 400n)
MSW out clk in 0 NMOD W=10u L=1u
CH out 0 2p
RLK out 0 100Meg
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 0.25n 2u
.end
