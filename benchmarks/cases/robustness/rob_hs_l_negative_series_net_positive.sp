L1=-2u series L2=6u (net 4 uH) between two 50 ohm resistors
* bench.id: rob_hs_l_negative_series_net_positive
* bench.group: robustness
* bench.title: L1=-2u series L2=6u (net 4 uH) between two 50 ohm resistors
* bench.probes: v(2), v(4)
* bench.expect: match
* bench.tags: hostile, value, l_negative, tran
* bench.est_rss_mib: 64
* bench.note: net inductance 4 uH, tau = 40 ns
V1 1 0 PULSE(0 5 0 20n 1n 1 2)
R1 1 2 50
L1 2 3 -2u
L2 3 4 6u
R2 4 0 50
.tran 1n 200n
.end
