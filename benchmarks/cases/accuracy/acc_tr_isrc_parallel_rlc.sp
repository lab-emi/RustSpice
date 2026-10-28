Parallel RLC driven by a current pulse
* bench.id: acc_tr_isrc_parallel_rlc
* bench.group: accuracy
* bench.title: Parallel RLC tank (f0 = 5.03 MHz, Q = 5) rung by a 1 mA current pulse, 50 steps per period
* bench.probes: v(out)
* bench.tol.norm: 7e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, rlc, isrc
* bench.est_rss_mib: 16
I1 0 out PULSE(0 1m 8n 4n 4n 40n 1)
R1 out 0 158.114
L1 out 0 1u
C1 out 0 1n
.tran 4n 1u
.end
