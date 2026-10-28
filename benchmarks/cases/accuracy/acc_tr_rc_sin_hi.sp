RC driven by a fast sine (20 steps per period)
* bench.id: acc_tr_rc_sin_hi
* bench.group: accuracy
* bench.title: RC low-pass driven by a 10x-corner sine, 20 steps per period
* bench.probes: v(out), v(in)
* bench.tol.norm: 3e-2
* bench.oracle: tran_lti
* bench.tags: linear, tran, sin
* bench.est_rss_mib: 16
* bench.note: Loosened to 3e-2: 20 steps per period is a coarse-step class where ngspice default is itself 8.5e-3 (RustSpice 1.7e-2: BDF1 first step).
V1 in 0 SIN(0 1 15.9155Meg)
R1 in out 1k
C1 out 0 100p
.tran 3.14n 3.14u
.end
