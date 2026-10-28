Resistor ladder with 20 stages, linear source sweep, 8193 points
* bench.id: perf_dc_lin_ladder20_8k
* bench.group: performance
* bench.title: Resistor ladder with 20 stages, linear source sweep, 8193 points
* bench.probes: v(n10), v(n20), i(v1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.oracle: dc_linear
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 22
* bench.est_rss_mib: 15
* bench.tags: linear, dc, sweep, ladder, jit_tier
* bench.note: purely linear 22-unknown sweep: one factorisation can serve every point and only the right-hand side changes (cached-LU regime); step 2^-10 V
V1 n0 0 0
Rs1 n0 n1 1k
Rp1 n1 0 5k
Rs2 n1 n2 1k
Rp2 n2 0 5k
Rs3 n2 n3 1k
Rp3 n3 0 2k
Rs4 n3 n4 1k
Rp4 n4 0 5k
Rs5 n4 n5 1k
Rp5 n5 0 5k
Rs6 n5 n6 1k
Rp6 n6 0 2k
Rs7 n6 n7 1k
Rp7 n7 0 5k
Rs8 n7 n8 1k
Rp8 n8 0 5k
Rs9 n8 n9 1k
Rp9 n9 0 2k
Rs10 n9 n10 1k
Rp10 n10 0 5k
Rs11 n10 n11 1k
Rp11 n11 0 5k
Rs12 n11 n12 1k
Rp12 n12 0 2k
Rs13 n12 n13 1k
Rp13 n13 0 5k
Rs14 n13 n14 1k
Rp14 n14 0 5k
Rs15 n14 n15 1k
Rp15 n15 0 2k
Rs16 n15 n16 1k
Rp16 n16 0 5k
Rs17 n16 n17 1k
Rp17 n17 0 5k
Rs18 n17 n18 1k
Rp18 n18 0 2k
Rs19 n18 n19 1k
Rp19 n19 0 5k
Rs20 n19 n20 1k
Rp20 n20 0 5k
.dc V1 0 8 0.0009765625
.end
