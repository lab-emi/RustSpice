MOSFET model with VTO=abc
* bench.id: rob_hs_mos_vto_abc
* bench.group: robustness
* bench.title: MOSFET model with VTO=abc
* bench.probes: v(d), i(vd)
* bench.expect: loud_error
* bench.tags: hostile, nonlinear, mos, model, parser, silent_risk
* bench.est_rss_mib: 64
* bench.note: refusing is the correct behaviour (abc is not a number, there is no unique truth). Sibling of rob_hs_mos_kp_nan with a different parameter and a plain word instead of a non-finite literal: RustSpice drops any unparsable value token of a model card. Level-1 saturation check: vds = 3 V > vov, id = 0.5*KP*W/L*(Vgs - VTO)^2 = 0.845 mA for VTO = 0.7 and 2.0 mA for the dropped VTO = 0 (KP = 1e-4, W/L = 10) [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
VD d 0 3
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(VTO=abc KP=1e-4)
.op
.end
