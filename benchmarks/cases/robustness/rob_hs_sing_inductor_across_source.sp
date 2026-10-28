inductor directly across a voltage source (DC short)
* bench.id: rob_hs_sing_inductor_across_source
* bench.group: robustness
* bench.title: inductor directly across a voltage source (DC short)
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: caught_panic_noise, hostile, singular, dc
* bench.est_rss_mib: 64
* bench.note: at DC the inductor is a 0 V branch and V1 forces 5 V across the same two nodes: 5 = 0, no solution exists, so refusing is correct. ngspice-47 warns "singular matrix" and prints arbitrary branch currents (i(l1)=0.05 A, i(v1)=-0.055 A) that satisfy no equation; RustSpice reports SingularMatrix. Same class as sing_vsource_loop_inconsistent. Decisive: RustSpice right
V1 1 0 5
L1 1 0 1m
R1 1 0 1k
.op
.end
