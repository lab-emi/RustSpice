R value 1e-400 (underflows to zero: a short to ground)
* bench.id: rob_hs_val_r_denormal_1e-400
* bench.group: robustness
* bench.title: R value 1e-400 (underflows to zero: a short to ground)
* bench.probes: v(1), v(2), i(v1)
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.tags: hostile, value, underflow, r_zero
* bench.est_rss_mib: 64
* bench.note: ngspice clamps R below 1e-12 ohm to 1e-12, RustSpice to 1e-18; v(2) is ~0 either way (5e-15 vs 5e-21)
V1 1 0 5
R1 1 2 1k
R2 2 0 1e-400
.op
.end
