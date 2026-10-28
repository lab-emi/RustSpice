ic on a node that does not exist
* bench.id: rob_hs_ic_missing_node
* bench.group: robustness
* bench.title: .ic on a node that does not exist next to a valid .ic v(2)=1
* bench.probes: v(2)
* bench.expect: match
* bench.tags: hostile, ic, tran, lenient_ok
* bench.est_rss_mib: 64
* bench.note: ngspice warns 'IC on non-existent node - nosuch, ignored' and starts v(2) at the valid 1 V; both tools then charge the capacitor from 1 V towards 5 V (2.573 V at 500 us). The valid .ic makes the transient non-trivial so a mis-applied or crashing bogus card would show (the old deck ran a constant 5 V trace)
V1 1 0 5
R1 1 2 1k
C1 2 0 1u
.ic v(2)=1 v(nosuch)=3
.tran 20u 500u
.end
