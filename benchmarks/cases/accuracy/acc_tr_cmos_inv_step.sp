CMOS inverter step response
* bench.id: acc_tr_cmos_inv_step
* bench.group: accuracy
* bench.title: CMOS inverter (W/L 20u/1u over 10u/1u, 3.3 V) driving 50 fF, 0.1 ns input edges, tstep 10 ps
* bench.probes: v(out)
* bench.tol.norm: 1e-2
* bench.tags: nonlinear, tran, mos, cmos
* bench.est_rss_mib: 16
VDD vdd 0 DC 3.3
VIN in 0 PULSE(0 3.3 0.2n 0.1n 0.1n 2n 4n)
MP1 out in vdd vdd PMOD W=20u L=1u
MN1 out in 0 0 NMOD W=10u L=1u
CL out 0 50f
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 10p 4n
.end
