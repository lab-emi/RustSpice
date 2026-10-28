NMOS instance W/L from .param names (W={wv} L={lv})
* bench.id: cap_dev_mos_w_l_param_ref
* bench.group: capability
* bench.title: NMOS instance W/L from .param names (W={wv} L={lv})
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, instance_param, dc, param
.param wv=20u
.param lv=2u
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W={wv} L={lv}
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VDS 0 5 0.1
.end
