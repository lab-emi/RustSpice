3-stage CMOS ring oscillator with a start-up current kick
* bench.id: acc_tr_ring3
* bench.group: accuracy
* bench.title: 3-stage CMOS ring oscillator (100 fF per stage, T ~ 0.45 ns) kicked by a 0.1 ns current pulse, tstep 2 ps
* bench.probes: v(n1), v(n2)
* bench.tol.norm: 5e-3
* bench.series.tstep: 16p 8p 4p 2p 1p
* bench.size: 3
* bench.tags: nonlinear, tran, mos, cmos, oscillator, series
* bench.est_rss_mib: 16
* bench.note: Series: the nominal member (2 ps) carries the verdict; the coarser 16 ps and 8 ps members are info and RustSpice is the more accurate tool there (6.4e-2/1.4e-2 vs ngspice as-is 8.0e-2/2.2e-2).
VDD vdd 0 DC 3.3
MP1 n1 n3 vdd vdd PMOD W=20u L=1u
MN1 n1 n3 0 0 NMOD W=10u L=1u
CL1 n1 0 100f
MP2 n2 n1 vdd vdd PMOD W=20u L=1u
MN2 n2 n1 0 0 NMOD W=10u L=1u
CL2 n2 0 100f
MP3 n3 n2 vdd vdd PMOD W=20u L=1u
MN3 n3 n2 0 0 NMOD W=10u L=1u
CL3 n3 0 100f
IK 0 n1 PULSE(0 1m 0.1n 20p 20p 100p 1)
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 2p 2.5n
.end
