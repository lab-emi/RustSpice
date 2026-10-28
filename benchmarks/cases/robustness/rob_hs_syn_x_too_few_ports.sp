X call with fewer nodes than the .subckt has ports
* bench.id: rob_hs_syn_x_too_few_ports
* bench.group: robustness
* bench.title: X call with fewer nodes than the .subckt has ports
* bench.probes: v(1), v(2)
* bench.expect: loud_error
* bench.tags: hostile, syntax, subckt, ngspice_crash
* bench.est_rss_mib: 64
.subckt s a b c
R1 a b 1k
R2 b c 1k
.ends
V1 1 0 5
X1 1 2 s
R9 2 0 1k
.op
.end
