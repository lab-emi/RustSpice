Diode peak detector
* bench.id: acc_tr_diode_peak_detector
* bench.group: accuracy
* bench.title: Peak detector: 5 V / 10 kHz sine, diode into 100 nF with a 100 k bleed, 100 steps per period
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.tags: nonlinear, tran, diode, peak_detector
* bench.est_rss_mib: 16
V1 in 0 SIN(0 5 10k)
D1 in out DMOD
C1 out 0 100n
R1 out 0 100k
.model DMOD D(IS=1e-14 N=1.0)
.tran 1u 1m
.end
