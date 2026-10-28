three zero-ohm resistors in a loop hanging off a divider
* bench.id: rob_hs_r_zero_triangle
* bench.group: robustness
* bench.title: three zero-ohm resistors in a loop hanging off a divider
* bench.probes: v(1), v(2), v(3), v(4), i(v1)
* bench.tol.rel: 5e-2
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.tags: hostile, value, r_zero, dc, silent_risk
* bench.est_rss_mib: 64
* bench.note: exact: v(2)=v(3)=v(4)=2.5 V, i(v1)=-2.5 mA; ngspice (R clamped to 1e-12 with a warning per resistor) gives 2.56 V and -2.44 mA (2.4% off), an artefact of its own floor (RustSpice with explicit R=1e-12 gives the same values), hence tol.rel 5e-2
V1 1 0 5
R1 1 2 1k
R2 2 3 0
R3 3 4 0
R4 4 2 0
R5 3 0 1k
.op
.end
