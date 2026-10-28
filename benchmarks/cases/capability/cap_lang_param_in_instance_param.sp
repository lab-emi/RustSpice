Parameter used as an instance parameter value
* bench.id: cap_lang_param_in_instance_param
* bench.group: capability
* bench.title: M1 ... W={wn} L={lch} takes the device size from .param (wn = 20u, lch = 1u)
* bench.probes: v(d)
* bench.tags: nonlinear, op, param, mos
.param wn=20u lch=1u
VDD vdd 0 DC 3
VG g 0 DC 1.5
R1 vdd d 10k
M1 d g 0 0 NM W={wn} L={lch}
.model NM NMOS(VTO=0.7 KP=100u)
.op
.end
