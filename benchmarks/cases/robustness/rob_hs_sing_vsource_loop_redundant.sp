two identical voltage sources in parallel
* bench.id: rob_hs_sing_vsource_loop_redundant
* bench.group: robustness
* bench.title: two identical voltage sources in parallel
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: caught_panic_noise, hostile, singular, dc
* bench.est_rss_mib: 64
* bench.note: redundant sources make the MNA matrix singular; ngspice also fails; RustSpice prints caught faer panics to stderr
V1 1 0 5
V2 1 0 5
R1 1 0 1k
.op
.end
