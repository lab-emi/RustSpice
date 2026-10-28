NMOS model parameter from .param names {vt}/{kpv}
* bench.id: cap_dev_mos_model_param_ref
* bench.group: capability
* bench.title: NMOS model parameter from .param names {vt}/{kpv}
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, model_card, dc, param
.param vt=0.9
.param kpv=80u
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO={vt} KP={kpv})
.dc VDS 0 5 0.1
.end
