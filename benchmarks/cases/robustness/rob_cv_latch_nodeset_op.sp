Cross-coupled CMOS latch with .nodeset v(q)=3.3 v(qb)=0 selecting the stable state
* bench.id: rob_cv_latch_nodeset_op
* bench.group: robustness
* bench.title: Cross-coupled CMOS latch with .nodeset v(q)=3.3 v(qb)=0 selecting the stable state
* bench.probes: v(q), v(qb)
* bench.expect: match
* bench.tags: nonlinear, bistable, nodeset, silent_risk, op
* bench.note: ngspice-47 honours the nodeset: q=3.3 V, qb=3.2e-9 V. Both results are legal DC solutions, but the one asked for is the stable one. Calibration evidence: an independent dense f64 Newton finds exactly three roots (q,qb) = (3.3,0), (0,3.3) and the metastable (1.5975393,1.5975393) V, and polishing from the nodeset state gives (3.3,0); RustSpice prints identical numbers with and without the .nodeset line, so the directive is silently ignored. The reference of v(qb) (3.18e-9 V) is below the floor, so that probe is flagged vacuous, but the harness still reports RustSpice's 1.6 V deviation on it as a failure; v(q) is the non-vacuous check. The low rail itself is comparable across the tools: RustSpice put in the stable state (transient started with .ic v(q)=3.3 v(qb)=0, run to 2 us) settles at 3.173e-9 V against 3.183e-9 V from tightly converged ngspice, so the tolerance would be met once the nodeset is honoured. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
Vdd vdd 0 3.3
M1 q qb 0 0 nm w=2u l=0.5u
M2 q qb vdd vdd pm w=4u l=0.5u
M3 qb q 0 0 nm w=2u l=0.5u
M4 qb q vdd vdd pm w=4u l=0.5u
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.7 kp=40u lambda=0.02)
.nodeset v(q)=3.3 v(qb)=0
.op
.end
