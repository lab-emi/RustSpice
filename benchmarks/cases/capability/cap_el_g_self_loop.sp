VCCS with its own output as control (a 2 mS conductance)
* bench.id: cap_el_g_self_loop
* bench.group: capability
* bench.title: VCCS with its own output as control (a 2 mS conductance)
* bench.probes: v(a)
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: controlled, op, vccs, self_loop
* bench.note: G1 a 0 a 0 2m draws 2 mS*v(a) from a: v(a)=0.5 V. ngspice default gmin (1e-12 S) shifts the result by 2.5e-10 V, far below the default tolerance.
I1 0 a 1m
G1 a 0 a 0 2m
.op
.end
