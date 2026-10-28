tran 1p 1e-18 (analysis shorter than one femtosecond)
* bench.id: rob_hs_tran_tiny_stop
* bench.group: robustness
* bench.title: .tran 1p 1e-18 (analysis shorter than one femtosecond)
* bench.probes: v(1)
* bench.floor: 1e-12
* bench.expect: match
* bench.tags: hostile, tran, args, scale
* bench.est_rss_mib: 64
* bench.note: analysis window 1e-18 s << tstep 1p: only the first point and the t=tstop point exist. Both tools give v(1)(tstop) = 5e-9 V (= 5 V * 1e-18/1e-9), compared with floor 1e-12 so the probe is not vacuous. v(2) is not probed: it is 2.5e-21 V (exact and ngspice) versus 5e-21 V for RustSpice (single backward-Euler step), far below any floor
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1n
.tran 1p 1e-18
.end
