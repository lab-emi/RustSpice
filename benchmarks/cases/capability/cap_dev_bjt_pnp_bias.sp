PNP BJT bias stage (IS=1e-15, BF=100)
* bench.id: cap_dev_bjt_pnp_bias
* bench.group: capability
* bench.title: PNP BJT bias stage (IS=1e-15, BF=100)
* bench.probes: v(c), v(b)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: Q card refused loudly (exit 1, ParseError `Unsupported device card Q1`); ngspice solves it: v(b)=4.285414 V, v(c)=0.996609 V
* bench.tags: bjt, op, unsupported
* bench.note: reference verified independently: hand-solved Ebers-Moll mirror of the NPN deck (python fsolve) gives v(b)=4.285414 v(c)=0.996608 = tight ngspice to 1e-6 V; ngspice default tolerances are 8e-5 V off the tight value
VCC vcc 0 5
RB b 0 430k
RC c 0 1k
Q1 c b vcc QP
.model QP PNP(IS=1e-15 BF=100)
.op
.end
