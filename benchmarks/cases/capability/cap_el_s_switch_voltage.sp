Voltage-controlled switch S (closed, ron=1, roff=1meg)
* bench.id: cap_el_s_switch_voltage
* bench.group: capability
* bench.title: Voltage-controlled switch S (closed, ron=1, roff=1meg)
* bench.probes: v(out)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the S card is refused at parse time (`Unsupported device card S1`, exit 1, no panic) where ngspice computes v(out) = 4.995 V; the SW .model card is accepted silently
* bench.tags: switch, op, unsupported
* bench.note: Hand value: control 1 V > vt 0.5 V so the switch is closed (ron = 1), v(out) = 5*1k/(1k+1) = 4.99500 V; ngspice-47 returns 4.9950049950. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 5
V2 ctl 0 1
S1 in out ctl 0 swm
R1 out 0 1k
.model swm SW(vt=0.5 vh=0 ron=1 roff=1meg)
.op
.end
