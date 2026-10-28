X call of a subcircuit that does not exist
* bench.id: rob_hs_syn_x_undefined_subckt
* bench.group: robustness
* bench.title: X call of a subcircuit that does not exist
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, subckt, ngspice_crash
* bench.est_rss_mib: 64
* bench.note: ngspice-47 reports 'unknown subckt' and then segfaults (exit 139)
V1 1 0 5
X1 1 0 nosuch
.op
.end
