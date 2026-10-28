ic v(0)=3 (initial condition on the ground node) next to a valid v(2)=1
* bench.id: rob_hs_ic_on_ground
* bench.group: robustness
* bench.title: .ic v(0)=3 (initial condition on the ground node) next to a valid .ic v(2)=1
* bench.probes: v(2)
* bench.expect: match
* bench.tags: hostile, ic, tran, lenient_ok
* bench.note: ground cannot be given an initial condition and both tools ignore v(0)=3; the valid v(2)=1 makes the trace non-trivial (v(2) rises from 1 V towards 5 V, 2.573 V at 500 us) so a mis-applied ground IC would show; the old deck ran a constant 5 V trace
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
C1 2 0 1u
.ic v(2)=1 v(0)=3
.tran 20u 500u
.end
