MESFET (Z card, NMF model)
* bench.id: cap_dev_mesfet_z
* bench.group: capability
* bench.title: MESFET (Z card, NMF model)
* bench.probes: v(d)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: Z card refused loudly (exit 1, ParseError `Unsupported device card Z1`); ngspice solves it: v(d)=4.782609 V
* bench.tags: mesfet, op, unsupported
* bench.note: reference verified independently: Statz drain current BETA*Vov^2/(1+B*Vov)*tanh(ALPHA*Vds) with the ngspice defaults B=0.3 ALPHA=2 LAMBDA=0 hand-solved gives v(d)=4.782609 = tight ngspice to 1e-8 V
VDD vdd 0 5
RD vdd d 1k
VG g 0 -0.5
Z1 d g 0 ZN
.model ZN NMF(VTO=-1 BETA=1m)
.op
.end
