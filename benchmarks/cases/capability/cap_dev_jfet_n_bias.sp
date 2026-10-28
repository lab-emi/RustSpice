N-channel JFET self-bias (VTO=-2, BETA=1m)
* bench.id: cap_dev_jfet_n_bias
* bench.group: capability
* bench.title: N-channel JFET self-bias (VTO=-2, BETA=1m)
* bench.probes: v(d), v(s)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: J card refused loudly (exit 1, ParseError `Unsupported device card J1`); ngspice solves it: v(d)=7.955483 V, v(s)=1.022259 V
* bench.tags: jfet, op, unsupported
* bench.note: reference verified independently: hand-solved square-law JFET (BETA*(Vgs-VTO)^2*(1+LAMBDA*Vds), saturation, self-bias RS=1k) gives v(d)=7.955483 v(s)=1.022259 = tight ngspice to 2e-8 V
VDD vdd 0 10
RD vdd d 2k
RS s 0 1k
J1 d 0 s JN
.model JN NJF(VTO=-2 BETA=1m LAMBDA=0.01)
.op
.end
