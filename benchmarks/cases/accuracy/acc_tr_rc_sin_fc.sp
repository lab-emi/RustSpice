RC driven by a sine at the corner frequency
* bench.id: acc_tr_rc_sin_fc
* bench.group: accuracy
* bench.title: RC low-pass driven by a sine at the -3 dB corner (1.59 MHz), 40 steps per period, DC offset 0.5 V
* bench.probes: v(out)
* bench.tol.norm: 3e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, sin
* bench.est_rss_mib: 16
V1 in 0 SIN(0.5 1 1.59155Meg)
R1 in out 1k
C1 out 0 100p
.tran 15.7n 15.7u
.end
