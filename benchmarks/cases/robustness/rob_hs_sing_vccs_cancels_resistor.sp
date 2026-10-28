VCCS of -1 S exactly cancels a 1 ohm resistor
* bench.id: rob_hs_sing_vccs_cancels_resistor
* bench.group: robustness
* bench.title: VCCS of -1 S exactly cancels a 1 ohm resistor
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: hostile, singular, dc, numeric
* bench.est_rss_mib: 64
* bench.note: KCL at node 1: v/1 = 1 mA + 1*v (the VCCS injects exactly the current the resistor draws), i.e. 0 = 1 mA: no solution exists, so refusing is the correct behaviour. ngspice-47 warns "singular matrix" and prints the gmin artefact v(1) = 1e9 V (arbitrary, not a solution); RustSpice reports SingularMatrix and exits 1. Decisive: RustSpice right
I1 0 1 1m
R1 1 0 1
G1 0 1 1 0 1
.op
.end
