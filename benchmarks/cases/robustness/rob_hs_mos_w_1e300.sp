MOSFET with W=1e300 (1e301 A drain current)
* bench.id: rob_hs_mos_w_1e300
* bench.group: robustness
* bench.title: MOSFET with W=1e300 (1e301 A drain current)
* bench.probes: v(d), i(vd)
* bench.expect: match
* bench.tags: hostile, nonlinear, mos, value, overflow
* bench.est_rss_mib: 64
* bench.note: both tools agree on i(vd) = -8.45e301 A (finite, near the overflow limit)
VD d 0 3
VG g 0 2
M1 d g 0 0 NM W=1e300 L=1u
.model NM NMOS(VTO=0.7 KP=100u)
.op
.end
