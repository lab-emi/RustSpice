Floating 10 V / 1 kHz source with 10 ohm series resistance feeding a diode bridge (100 uF / 1k load, 100k common-mode resistors)
* bench.id: rob_cv_rect_bridge_rs_10v_tran
* bench.group: robustness
* bench.title: Floating 10 V / 1 kHz source with 10 ohm series resistance feeding a diode bridge (100 uF / 1k load, 100k common-mode resistors)
* bench.probes: v(p)
* bench.tol.norm: 5e-4
* bench.expect: match
* bench.tags: nonlinear, rectifier, floating_source, tran
* bench.note: Same bridge as rob_cv_rect_bridge_rs_5v_tran at 10 V amplitude (steady-state peak 6.87 V). Reference verified: independent scipy Radau integration (f64, rtol 1e-10) agrees with tight ngspice to 1.2e-7 of the 6.87 V swing (ngspice as-is 4.6e-5). Mechanism check: adding a +20 V or a +-20 V dummy source with a 1k load (clamp widened, no other change) lets RustSpice finish all 1001 points at 4.4e-5 of the swing from Radau, so accuracy is fine once the clamp is out of the way; the failure instant matches the first time the ngspice trace p(t) meets the clamp upper bound (5.4706 V against a bound of 5.4706 V at 2.49251 ms).
Vs a1 b SIN(0 10 1k)
Rs a1 a 10
Rg1 a 0 100k
Rg2 b 0 100k
D1 a p dm
D2 b p dm
D3 0 a dm
D4 0 b dm
C1 p 0 100u
R1 p 0 1k
.model dm D(is=1e-14 n=1)
.tran 5u 5m
.end
