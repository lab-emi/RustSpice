two RC sections with tau = 1 fs and 1 ms (stiffness 1e12), 1 us fixed grid
* bench.id: rob_hs_num_stiffness_1e12
* bench.group: robustness
* bench.title: two RC sections with tau = 1 fs and 1 ms (stiffness 1e12), 1 us fixed grid
* bench.probes: v(2), v(3)
* bench.floor: 1e-9
* bench.oracle: tran_lti
* bench.expect: match
* bench.tags: numeric, stiff, tran
* bench.est_rss_mib: 64
* bench.note: v(2) follows the 1 fs section (1 V), v(3) charges through 1 Mohm into 1 mF (tau = 1000 s): v(3) = 1e-6 V at 1 ms
V1 1 0 PULSE(0 1 0 1p 1p 1 2)
R1 1 2 1
C1 2 0 1f
R2 2 3 1e6
C2 3 0 1m
.tran 1u 1m
.end
