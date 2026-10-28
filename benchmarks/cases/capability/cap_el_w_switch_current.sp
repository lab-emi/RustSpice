Current-controlled switch W (closed by the current through Vs)
* bench.id: cap_el_w_switch_current
* bench.group: capability
* bench.title: Current-controlled switch W (closed by the current through Vs)
* bench.probes: v(out)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the W card is refused at parse time (`Unsupported device card W1`, exit 1, no panic) where ngspice computes v(out) = 4.995 V; the CSW .model card is accepted silently
* bench.tags: switch, op, cccs, unsupported
* bench.note: Hand value: I1 pushes 1 mA into node a and out through the + terminal of Vs, so i(Vs) = +1 mA (SPICE sign: into the + terminal; ngspice-47 reports +1e-3). |i| = 1 mA > it = 0.5 mA so the switch is closed (ron = 1) and v(out) = 5*1k/(1k+1) = 4.9950 V. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 5
Vs a 0 0
I1 0 a 1m
W1 in out Vs swc
R1 out 0 1k
.model swc CSW(it=0.5m ih=0 ron=1 roff=1meg)
.op
.end
