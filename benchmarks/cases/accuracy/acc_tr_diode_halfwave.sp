Half-wave rectifier with RC load
* bench.id: acc_tr_diode_halfwave
* bench.group: accuracy
* bench.title: Half-wave rectifier (10 V, 1 kHz) into 1 k / 1 uF, 100 steps per period
* bench.probes: v(out)
* bench.tol.norm: 1e-2
* bench.tags: nonlinear, tran, diode, rectifier
* bench.note: Reference verified: tight ngspice agrees with an independent adaptive Radau IIA integration to 1.2e-4 of the swing (ngspice as-is: 1.1e-2).
* bench.est_rss_mib: 16
V1 in 0 SIN(0 10 1k)
D1 in out DMOD
R1 out 0 1k
C1 out 0 1u
.model DMOD D(IS=1e-14 N=1.0)
.tran 10u 5m
.end
