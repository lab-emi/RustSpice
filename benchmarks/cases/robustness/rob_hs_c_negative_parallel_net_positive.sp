C1=-0.5n parallel C2=1.5n (net 1 nF) RC step, tau = 1 us
* bench.id: rob_hs_c_negative_parallel_net_positive
* bench.group: robustness
* bench.title: C1=-0.5n parallel C2=1.5n (net 1 nF) RC step, tau = 1 us
* bench.probes: v(2)
* bench.expect: match
* bench.tags: hostile, value, c_negative, tran
* bench.est_rss_mib: 64
* bench.note: closed form: v(2) = 5*(1-exp(-t/1us)) = 4.7511 V at 3 us (the 1 ns edge changes it by < 1e-3 V); ngspice with tight options peaks at 4.75095 V, RustSpice at 4.75106 V; a clamp of C<=0 to zero or abs(C) changes tau and fails
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 -0.5n
C2 2 0 1.5n
.tran 10n 3u
.end
