Resistor ladder, 28 stages of 1k/1k: node voltage decays from 0.38 V to 2.0e-12 V
* bench.id: acc_dc_lin_ladder_decay_dyn_range
* bench.group: accuracy
* bench.title: Resistor ladder, 28 stages of 1k/1k: node voltage decays from 0.38 V to 2.0e-12 V
* bench.probes: v(n1), v(n7), v(n14), v(n21), v(n28)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-24
* bench.floor: 1e-16
* bench.oracle: dc_linear
* bench.size: 29
* bench.tags: linear, op, dynamic_range, ladder
* bench.note: deep nodes must be right in relative terms: tol.abs 1e-24 so the 2.0e-12 V node is held to 1e-9 relative
V1 n0 0 1
Rs1 n0 n1 1k
Rp1 n1 0 1k
Rs2 n1 n2 1k
Rp2 n2 0 1k
Rs3 n2 n3 1k
Rp3 n3 0 1k
Rs4 n3 n4 1k
Rp4 n4 0 1k
Rs5 n4 n5 1k
Rp5 n5 0 1k
Rs6 n5 n6 1k
Rp6 n6 0 1k
Rs7 n6 n7 1k
Rp7 n7 0 1k
Rs8 n7 n8 1k
Rp8 n8 0 1k
Rs9 n8 n9 1k
Rp9 n9 0 1k
Rs10 n9 n10 1k
Rp10 n10 0 1k
Rs11 n10 n11 1k
Rp11 n11 0 1k
Rs12 n11 n12 1k
Rp12 n12 0 1k
Rs13 n12 n13 1k
Rp13 n13 0 1k
Rs14 n13 n14 1k
Rp14 n14 0 1k
Rs15 n14 n15 1k
Rp15 n15 0 1k
Rs16 n15 n16 1k
Rp16 n16 0 1k
Rs17 n16 n17 1k
Rp17 n17 0 1k
Rs18 n17 n18 1k
Rp18 n18 0 1k
Rs19 n18 n19 1k
Rp19 n19 0 1k
Rs20 n19 n20 1k
Rp20 n20 0 1k
Rs21 n20 n21 1k
Rp21 n21 0 1k
Rs22 n21 n22 1k
Rp22 n22 0 1k
Rs23 n22 n23 1k
Rp23 n23 0 1k
Rs24 n23 n24 1k
Rp24 n24 0 1k
Rs25 n24 n25 1k
Rp25 n25 0 1k
Rs26 n25 n26 1k
Rp26 n26 0 1k
Rs27 n26 n27 1k
Rp27 n27 0 1k
Rs28 n27 n28 1k
Rp28 n28 0 1k
.op
.end
