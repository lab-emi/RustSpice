R value 1e999 (overflows f64 to infinity = open circuit)
* bench.id: rob_hs_val_r_1e999
* bench.group: robustness
* bench.title: R value 1e999 (overflows f64 to infinity = open circuit)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: hostile, value, nonfinite, overflow
* bench.est_rss_mib: 64
* bench.note: both tools read 1e999 as an open circuit: v(2)=5 V, i(v1)=0
V1 1 0 5
R1 1 2 1k
R2 2 0 1e999
.op
.end
