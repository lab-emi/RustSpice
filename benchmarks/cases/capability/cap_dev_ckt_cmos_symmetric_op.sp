Symmetric CMOS inverter at Vin=VDD/2 (NMOS and PMOS mirror images, closed form)
* bench.id: cap_dev_ckt_cmos_symmetric_op
* bench.group: capability
* bench.title: Symmetric CMOS inverter at Vin=VDD/2 (NMOS and PMOS mirror images, closed form)
* bench.probes: v(out), i(vdd)
* bench.expect: match
* bench.tags: textbook, op, cmos, symmetry, high_gain
* bench.note: new deck: KP_n*W_n = KP_p*W_p (100u*10u = 25u*40u), |VTO|=0.7, LAMBDA=0.02, so by symmetry v(out)=VDD/2=1.5 V exactly and i(vdd)=-0.5*(KP*W/L)*(VDD/2-0.7)^2*(1+LAMBDA*1.5)=-3.296e-4 A (closed form, both simulators agree to 1e-14); the inverter sits in its high-gain region, so any NMOS/PMOS sign or body-reference asymmetry moves v(out) by many mV (a 5% KP_p change moves it by 0.8 V)
VDD vdd 0 3
VIN in 0 1.5
MP out in vdd vdd PM W=40u L=1u
MN out in 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.model PM PMOS(LEVEL=1 VTO=-0.7 KP=25u LAMBDA=0.02)
.op
.end
