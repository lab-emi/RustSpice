Depletion-load NMOS inverter VTC (VTO=-1.5 V load, gate tied to its source, body effect)
* bench.id: cap_dev_mos_depletion_load_vtc
* bench.group: capability
* bench.title: Depletion-load NMOS inverter VTC (VTO=-1.5 V load, gate tied to its source, body effect)
* bench.probes: v(out), i(vdd)
* bench.expect: match
* bench.tags: textbook, dc, nmos, vtc, body_effect, depletion
* bench.note: new deck: negative-VTO NMOS (depletion) as a load with source=out, so the load's Vsb equals v(out). Own python level-1 solve (body effect, LAMBDA in triode and saturation, gmin across drain-source) equals RustSpice to 7.7e-10 V on all 51 points; tight ngspice is 4e-7 V away from it in the high-gain region (its own accuracy; ngspice default vs tight 5.7e-4 V), well inside tol.rel 1e-6 of a 1..4 V output
VDD vdd 0 5
VIN in 0 0
M2 vdd out out 0 ND W=4u L=8u
M1 out in 0 0 NE W=20u L=2u
.model NE NMOS(LEVEL=1 VTO=0.8 KP=100u LAMBDA=0.02 GAMMA=0.4 PHI=0.7)
.model ND NMOS(LEVEL=1 VTO=-1.5 KP=100u LAMBDA=0.02 GAMMA=0.4 PHI=0.7)
.dc VIN 0 5 0.1
.end
