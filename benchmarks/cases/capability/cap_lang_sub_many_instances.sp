Many instances of one subcircuit
* bench.id: cap_lang_sub_many_instances
* bench.group: capability
* bench.title: 40 instances of the same two-resistor stage chained in series
* bench.probes: v(n1), v(n20), v(n40)
* bench.tags: linear, op, subckt, scale
.subckt STAGE in out
R1 in out 1k
R2 out 0 20k
.ends STAGE
V1 n0 0 DC 5
X1 n0 n1 STAGE
X2 n1 n2 STAGE
X3 n2 n3 STAGE
X4 n3 n4 STAGE
X5 n4 n5 STAGE
X6 n5 n6 STAGE
X7 n6 n7 STAGE
X8 n7 n8 STAGE
X9 n8 n9 STAGE
X10 n9 n10 STAGE
X11 n10 n11 STAGE
X12 n11 n12 STAGE
X13 n12 n13 STAGE
X14 n13 n14 STAGE
X15 n14 n15 STAGE
X16 n15 n16 STAGE
X17 n16 n17 STAGE
X18 n17 n18 STAGE
X19 n18 n19 STAGE
X20 n19 n20 STAGE
X21 n20 n21 STAGE
X22 n21 n22 STAGE
X23 n22 n23 STAGE
X24 n23 n24 STAGE
X25 n24 n25 STAGE
X26 n25 n26 STAGE
X27 n26 n27 STAGE
X28 n27 n28 STAGE
X29 n28 n29 STAGE
X30 n29 n30 STAGE
X31 n30 n31 STAGE
X32 n31 n32 STAGE
X33 n32 n33 STAGE
X34 n33 n34 STAGE
X35 n34 n35 STAGE
X36 n35 n36 STAGE
X37 n36 n37 STAGE
X38 n37 n38 STAGE
X39 n38 n39 STAGE
X40 n39 n40 STAGE
.op
.end
