SIN with frequency 1e999 (infinity)
* bench.id: rob_hs_src_sin_freq_inf
* bench.group: robustness
* bench.title: SIN with frequency 1e999 (infinity)
* bench.probes: v(2)
* bench.expect: loud_error
* bench.tags: hostile, source, nonfinite, overflow, tran, silent_risk
* bench.est_rss_mib: 64
* bench.note: 1e309 behaves like 1e999 (NaN rows, exit 0; ngspice exits 1), while f = 1e6 gives finite output that ngspice solves too. The same source with a diode on node 2 (Newton path) is refused by RustSpice (exit 1, TransientFailure with residual NaN), so the linear transient path has no finiteness check; this is not a resource budget. The literal inf in the same position is a different defect (dropped as unparsable, SIN degrades to 0 V): rob_hs_src_sin_freq_inf_literal.
V1 1 0 SIN(0 1 1e999)
R1 1 2 1k
C1 2 0 100p
.tran 10n 2u
.end
