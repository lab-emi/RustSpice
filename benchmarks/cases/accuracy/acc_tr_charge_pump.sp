Dickson charge pump
* bench.id: acc_tr_charge_pump
* bench.group: accuracy
* bench.title: Three-diode Dickson pump (5 V supply and clock with 1 us edges, 100 nF stages, 1 Meg load) pumping above the supply
* bench.probes: v(n1), v(n3)
* bench.tol.norm: 5e-3
* bench.tags: nonlinear, tran, diode, charge_pump
* bench.note: tol.norm was 2e-2 and is tightened to 5e-3 (SUITE_SPEC 13.4.2): 2e-2 was 3x the as-is ngspice error (6.9e-3 against the reference) and 360x the reference error (5.6e-5) without a proof, and hid RustSpice's own 1.6e-2 error once the clamp is out of the way. Reference verified: tight ngspice agrees with an independent adaptive Radau IIA integration to 5.6e-5 (v(n1)) and 1.6e-5 (v(n3)) of the swing; ngspice as-is is 7.8e-3 and 2.2e-3 off.
* bench.est_rss_mib: 16
Vdd vdd 0 5
Vclk clk 0 PULSE(0 5 0 1u 1u 4u 10u)
D1 vdd n1 DMOD
C1 n1 clk 100n
D2 n1 n2 DMOD
C2 n2 0 100n
D3 n2 n3 DMOD
C3 n3 clk 100n
R1 n3 0 1Meg
.model DMOD D(IS=1e-14 N=1.0)
.tran 50n 60u
.end
