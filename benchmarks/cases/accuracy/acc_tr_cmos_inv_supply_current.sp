CMOS inverter supply-current waveform
* bench.id: acc_tr_cmos_inv_supply_current
* bench.group: accuracy
* bench.title: Supply current i(vdd) of a CMOS inverter (shoot-through and load-charging spikes) for 0.1 ns input edges, tstep 10 ps
* bench.probes: i(vdd)
* bench.tol.norm: 1e-2
* bench.tags: nonlinear, tran, mos, cmos, branch_current
* bench.est_rss_mib: 16
* bench.note: Branch-current probe of a nonlinear circuit: ngspice default (2.1e-2) is less accurate than RustSpice (5.3e-3) here.
VDD vdd 0 DC 3.3
VIN in 0 PULSE(0 3.3 0.2n 0.1n 0.1n 2n 4n)
MP1 out in vdd vdd PMOD W=20u L=1u
MN1 out in 0 0 NMOD W=10u L=1u
CL out 0 50f
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 10p 4n
.end
