MOSFET model with an empty KP= value
* bench.id: rob_hs_mos_kp_empty
* bench.group: robustness
* bench.title: MOSFET model with an empty KP= value
* bench.probes: v(d), i(vd)
* bench.expect: loud_error
* bench.tags: hostile, nonlinear, mos, model, parser, silent_risk
* bench.est_rss_mib: 64
* bench.note: refusing is the correct behaviour (a model parameter without a value is malformed, there is no unique truth). Sibling of rob_hs_mos_kp_nan: the value token is missing instead of non-finite and RustSpice still drops it. Level-1 saturation check: vds = 3 V > vov, id = 0.5*KP*W/L*(Vgs - VTO)^2 = 0.169 mA for the default KP = 2e-5 (W/L = 10, VTO = 0.7) [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
VD d 0 3
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(VTO=0.7 KP=)
.op
.end
