Cross-coupled CMOS inverters with .nodeset v(q)=3.3 v(qb)=0: the nodeset selects the stable state
* bench.id: acc_dc_mos_latch_nodeset
* bench.group: accuracy
* bench.title: Cross-coupled CMOS inverters with .nodeset v(q)=3.3 v(qb)=0: the nodeset selects the stable state
* bench.probes: v(q), v(qb)
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .nodeset is accepted and ignored: RS returns the metastable 1.5548 V / 1.5548 V; ngspice returns 3.3 V / 3e-9 V (silent)
* bench.tags: mos, level1, op, bistable, nodeset, silent_risk
* bench.note: tol.abs default; the nodeset state is the one stable solution selected by the hint. Independent evidence: a dense f64 root search of the Level-1 equations finds exactly three DC solutions within the supply rails (re-checked with scipy fsolve from a 12 x 12 grid of starts), (q,qb) = (0,3.3), (1.5547568,1.5547568) and (3.3,0); RustSpice's 1.5547568 V is the metastable root (it prints the same numbers with and without the .nodeset line) and ngspice (3.2999999960, 3.18e-9) follows the nodeset. The reference of v(qb) is below the floor, so that probe is flagged vacuous, but the harness still reports RustSpice's 1.55 V deviation on it as a failure; v(q) is the non-vacuous check. The low rail is comparable across the tools (RustSpice in the stable state, via a transient started with .ic v(q)=3.3 v(qb)=0: 3.173e-9 V against 3.183e-9 V from tightly converged ngspice), so tol.abs 1e-9 is met once the nodeset is honoured
Vdd vdd 0 3.3
M1 q qb 0 0 nm w=2u l=0.5u
M2 q qb vdd vdd pm w=4u l=0.5u
M3 qb q 0 0 nm w=2u l=0.5u
M4 qb q vdd vdd pm w=4u l=0.5u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.model pm pmos(level=1 vto=-0.8 kp=40u gamma=0.5 phi=0.7 lambda=0.04)
.nodeset v(q)=3.3 v(qb)=0
.op
.end
