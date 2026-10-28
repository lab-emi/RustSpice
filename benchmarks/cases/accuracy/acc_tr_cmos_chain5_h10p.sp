5-stage CMOS inverter chain at tstep 10 ps
* bench.id: acc_tr_cmos_chain5_h10p
* bench.group: accuracy
* bench.title: 5-stage CMOS inverter chain (20 fF per stage, 0.1 ns input edge) at tstep 10 ps, twice the nominal step of acc_tr_cmos_chain5
* bench.probes: v(n1), v(n3), v(n5)
* bench.tol.norm: 3.5e-2
* bench.expect: either
* bench.size: 5
* bench.tags: nonlinear, tran, mos, cmos, ambiguous
* bench.est_rss_mib: 16
* bench.note: Ambiguous, so information only: both tools miss tol.norm 3.5e-2 at this step (RustSpice 8.3e-2 on v(n5) and 5.0e-2 on v(n3); ngspice as-is 4.4e-2 on v(n5) and 2.2e-2 on v(n1)), RustSpice is only 1.9x worse than ngspice, and the tolerance was set for the nominal 5 ps step, so neither tool can be judged wrong here. RustSpice's error is plain second-order trapezoid truncation (all PULSE corners lie on the 10 ps grid; its error falls 3.7-4.3x per step halving from 10 ps down, 1.9e-2 at 5 ps and 3.5e-4 at 0.625 ps), and the converged ngspice reference agrees with that refinement. This member was taken out of the acc_tr_cmos_chain5 series (a series shares one known_gap); the 20 ps member of that series, where RustSpice is 3.0x worse than ngspice, carries the gap.
VDD vdd 0 DC 3.3
VIN in1 0 PULSE(0.0 3.3 0.2n 0.1n 0.1n 2n 4n)
MP1 n1 in1 vdd vdd PMOD W=20u L=1u
MN1 n1 in1 0 0 NMOD W=10u L=1u
CL1 n1 0 20f
MP2 n2 n1 vdd vdd PMOD W=20u L=1u
MN2 n2 n1 0 0 NMOD W=10u L=1u
CL2 n2 0 20f
MP3 n3 n2 vdd vdd PMOD W=20u L=1u
MN3 n3 n2 0 0 NMOD W=10u L=1u
CL3 n3 0 20f
MP4 n4 n3 vdd vdd PMOD W=20u L=1u
MN4 n4 n3 0 0 NMOD W=10u L=1u
CL4 n4 0 20f
MP5 n5 n4 vdd vdd PMOD W=20u L=1u
MN5 n5 n4 0 0 NMOD W=10u L=1u
CL5 n5 0 20f
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 10p 2n
.end
