V source value 1e999 (infinity) with a diode load
* bench.id: rob_hs_val_v_1e999_diode
* bench.group: robustness
* bench.title: V source value 1e999 (infinity) with a diode load (nonlinear path)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, value, nonfinite
* bench.est_rss_mib: 64
* bench.note: Control for rob_hs_val_v_1e999. The same infinite source (the literal 1e999 parses to +inf), but the diode D1 on node 2 sends the operating point down the Newton path, which checks the iterate for finiteness: RustSpice exits 1 (ConvergenceFailure with residual NaN, verified) and ngspice-47 exits 1 as well (gmin and source stepping fail, verified). No finite operating point exists for a source of +inf V, so refusal is correct. The pair shows that the NaN output of rob_hs_val_v_1e999 comes from the linear path lacking a finiteness check, and it pins the refusal so that a fix on the linear path cannot regress this one.
V1 1 0 1e999
R1 1 2 1k
R2 2 0 1k
D1 2 0 dm
.model dm D(IS=1e-14)
.op
.end
