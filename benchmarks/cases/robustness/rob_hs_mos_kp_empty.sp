MOSFET model with an empty KP= value
* bench.id: rob_hs_mos_kp_empty
* bench.group: robustness
* bench.title: MOSFET model with an empty KP= value
* bench.probes: v(d), i(vd)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: the empty KP= is dropped silently and the default KP is used (exit 0, empty stderr): i(vd) = -0.169 mA, identical to KP=nan and to no KP at all; ngspice-47 refuses the model card (exit 1, 'Syntax error: letter [)]')
* bench.tags: hostile, nonlinear, mos, model, parser, silent_risk
* bench.est_rss_mib: 64
* bench.note: refusing is the correct behaviour (a model parameter without a value is malformed, there is no unique truth). Sibling of rob_hs_mos_kp_nan: the value token is missing instead of non-finite and RustSpice still drops it. Level-1 saturation check: vds = 3 V > vov, id = 0.5*KP*W/L*(Vgs - VTO)^2 = 0.169 mA for the default KP = 2e-5 (W/L = 10, VTO = 0.7)
VD d 0 3
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(VTO=0.7 KP=)
.op
.end
