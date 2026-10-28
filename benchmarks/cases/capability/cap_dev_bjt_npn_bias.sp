NPN BJT fixed-bias stage (IS=1e-15, BF=100)
* bench.id: cap_dev_bjt_npn_bias
* bench.group: capability
* bench.title: NPN BJT fixed-bias stage (IS=1e-15, BF=100)
* bench.probes: v(c), v(b)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: Q card refused loudly (exit 1, ParseError `Unsupported device card Q1`); ngspice solves it: v(b)=0.714586 V, v(c)=4.003392 V
* bench.tags: bjt, op, unsupported
* bench.note: reference verified independently: hand-solved Ebers-Moll (IS=1e-15 BF=100 BR=1, gmin 1e-12, python fsolve) gives v(b)=0.714586 v(c)=4.003392 = tight ngspice to 1e-6 V
VCC vcc 0 5
RB vcc b 430k
RC vcc c 1k
Q1 c b 0 QN
.model QN NPN(IS=1e-15 BF=100)
.op
.end
