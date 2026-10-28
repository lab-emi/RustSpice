param a={b+1} b={a+1} (cycle)
* bench.id: rob_hs_par_cycle
* bench.group: robustness
* bench.title: .param a={b+1} b={a+1} (cycle)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, param, cycle
* bench.est_rss_mib: 64
* bench.note: ngspice stops with "A level depth greater 1000 for dependent parameters is not supported". RustSpice refuses {a} because a param defined by an expression that starts with an identifier ({b+1}) is not evaluated at all, not because it detects the cycle
.param a={b+1} b={a+1}
V1 1 0 5
R1 1 2 {a}
R2 2 0 1k
.op
.end
