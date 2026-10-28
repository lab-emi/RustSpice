case: 1 kV source directly across a diode (exp overflow)
* bench.id: rob_hs_diode_forward_1kv_no_resistor
* bench.group: robustness
* bench.title: 1 kV source directly across a diode (exp overflow)
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: hostile, nonlinear, overflow, diode
* bench.est_rss_mib: 64
* bench.note: exp(1000/0.0259) overflows; ngspice reports 'DC solution failed'; no finite solution exists
V1 1 0 1000
D1 1 0 DM
.model DM D(IS=1e-14)
.op
.end
