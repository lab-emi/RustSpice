dc of a resistor (legal in ngspice: resistance sweep)
* bench.id: rob_hs_dc_source_is_resistor
* bench.group: robustness
* bench.title: .dc of a resistor (legal in ngspice: resistance sweep)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: hostile, dc, args, capability
* bench.est_rss_mib: 64
* bench.note: legal in ngspice (resistance sweep, swept variable named res-sweep): v(2) = 2.5, 3.3333, 3.75 V and i(v1) = -2.5, -1.667, -1.25 mA, equal to 5*R2/(1k+R2) (verified with tight options)
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.dc R2 1k 3k 1k
.end
