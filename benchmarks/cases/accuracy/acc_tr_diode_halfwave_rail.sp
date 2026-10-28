Half-wave rectifier with RC load, with a 10 V DC rail present in the netlist
* bench.id: acc_tr_diode_halfwave_rail
* bench.group: accuracy
* bench.title: Half-wave rectifier (10 V, 1 kHz) into 1 k / 1 uF with an unrelated 10 V rail so a DC-source-range clamp does not interfere
* bench.probes: v(out)
* bench.tol.norm: 1e-2
* bench.series.tstep: 40u 20u 10u 5u 2.5u 1u
* bench.tags: nonlinear, tran, diode, rectifier, series
* bench.est_rss_mib: 16
* bench.note: VDUM/RDUM are an unconnected-from-the-signal-path 10 V rail; they only exist to isolate accuracy from the clamp-rail defect tested by acc_tr_diode_halfwave. Series: the nominal member (10 us) carries the verdict; the coarser 40 us (25 steps per period) member is info and not a defect (RustSpice 3.8e-2, ngspice as-is 5.8e-2).
V1 in 0 SIN(0 10 1k)
D1 in out DMOD
R1 out 0 1k
C1 out 0 1u
VDUM dum 0 DC 10
RDUM dum 0 1k
.model DMOD D(IS=1e-14 N=1.0)
.tran 10u 5m
.end
