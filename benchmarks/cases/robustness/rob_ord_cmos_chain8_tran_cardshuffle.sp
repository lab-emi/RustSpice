8-stage CMOS inverter chain (20 fF loads), cards shuffled (seed 2)
* bench.id: rob_ord_cmos_chain8_tran_cardshuffle
* bench.group: robustness
* bench.title: 8-stage CMOS inverter chain (20 fF loads), cards shuffled (seed 2)
* bench.probes: v(n1), v(n2), v(n4), v(n6), v(n8)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.size: 12
* bench.est_rss_mib: 20
* bench.tags: nonlinear, mos, tran, ladder, card_order, control, regression_anchor
* bench.note: nonlinear control: the 5-stage circuit of acc_tr_cmos_chain5 extended to 8 stages, card order shuffled by random.Random(2); the Newton path refactorises each iteration and measured card-order deviations from the natural order are below 2.1e-14 of swing over 8 seeds; 1 ps steps put the natural-order error at under 1e-3 of swing (5 ps: 1.9e-2), hence tol.norm 5e-3; reference = converged ngspice
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
MN1 n1 in1 0 0 NMOD W=10u L=1u
MP7 n7 n6 vdd vdd PMOD W=20u L=1u
MP6 n6 n5 vdd vdd PMOD W=20u L=1u
MN5 n5 n4 0 0 NMOD W=10u L=1u
VDD vdd 0 DC 3.3
MP8 n8 n7 vdd vdd PMOD W=20u L=1u
CL1 n1 0 20f
CL2 n2 0 20f
MP5 n5 n4 vdd vdd PMOD W=20u L=1u
CL5 n5 0 20f
CL6 n6 0 20f
CL7 n7 0 20f
MN4 n4 n3 0 0 NMOD W=10u L=1u
MN6 n6 n5 0 0 NMOD W=10u L=1u
CL3 n3 0 20f
CL4 n4 0 20f
MN7 n7 n6 0 0 NMOD W=10u L=1u
CL8 n8 0 20f
MN2 n2 n1 0 0 NMOD W=10u L=1u
MP3 n3 n2 vdd vdd PMOD W=20u L=1u
MN3 n3 n2 0 0 NMOD W=10u L=1u
MP2 n2 n1 vdd vdd PMOD W=20u L=1u
MP4 n4 n3 vdd vdd PMOD W=20u L=1u
MN8 n8 n7 0 0 NMOD W=10u L=1u
MP1 n1 in1 vdd vdd PMOD W=20u L=1u
VIN in1 0 PULSE(0.0 3.3 0.2n 0.1n 0.1n 2n 4n)
.tran 1p 3.5n
.end
