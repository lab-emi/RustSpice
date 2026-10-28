MOSFET model with KP=nan
* bench.id: rob_hs_mos_kp_nan
* bench.group: robustness
* bench.title: MOSFET model with KP=nan
* bench.probes: v(d), i(vd)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: RS exits 0 with empty stderr and drops the unparsable token: KP=nan, KP=abc, KP=inf, an empty KP= and no KP at all give the identical i(vd) = -0.169 mA (default KP); ngspice-47 refuses the model card (exit 1, 'Undefined parameter [nan]')
* bench.tags: hostile, nonlinear, mos, model, nonfinite, silent_risk
* bench.est_rss_mib: 64
* bench.note: refusing is the correct behaviour (nan is not a model value, there is no unique truth). RustSpice drops ANY unparsable value token of a model card, not one particular MOS parameter: VTO=abc, VTO=nan and LAMBDA=abc are dropped as well while ngspice-47 exits 1 on every one of them; valid values (KP=1e-4, KP=0, KP=-1e-4) agree between the two tools, so this is not a dropped-model-parameter gap (slug retargeted from silent-ignore-model-param). Siblings: rob_hs_mos_vto_abc, rob_hs_mos_kp_empty
VD d 0 3
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(VTO=0.7 KP=nan)
.op
.end
