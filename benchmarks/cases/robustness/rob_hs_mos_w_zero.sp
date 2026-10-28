MOSFET with W=0 behind a drain resistor
* bench.id: rob_hs_mos_w_zero
* bench.group: robustness
* bench.title: MOSFET with W=0 behind a 1k drain resistor
* bench.probes: v(vdd), v(d), i(vd)
* bench.tol.abs: 1e-9
* bench.tol.rel: 1e-6
* bench.expect: match
* bench.tags: hostile, nonlinear, mos, value
* bench.est_rss_mib: 64
* bench.note: channel current is 0, so v(d) must stay at 3 V minus 1k*gmin leakage (2.999999997 V in both tools; a default width would pull it to about 2.155 V as W=10u does). i(vd) = -3e-12 A is below the floor and therefore vacuous; v(d) behind RD is the non-trivial probe (the old deck probed only source nodes)
VD vdd 0 3
RD vdd d 1k
VG g 0 2
M1 d g 0 0 NM W=0 L=1u
.model NM NMOS(VTO=0.7 KP=100u)
.op
.end
