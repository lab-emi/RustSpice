.nodeset selects one stable state of a latch
* bench.id: cap_lang_nodeset_latch_high
* bench.group: capability
* bench.title: cross-coupled CMOS latch with .nodeset v(q)=3 v(qb)=0 must settle with q high
* bench.probes: v(q), v(qb)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .nodeset ignored: RustSpice returns the metastable midpoint v(q) = v(qb) = 1.5 V (exit 0) where ngspice follows the nodeset to q = 2.999999999 V, qb = 1.3e-9 V
* bench.tags: nonlinear, op, nodeset, bistable
* bench.note: the symmetric latch (beta_n = beta_p) has three DC solutions (two stable ones, (3,0) and (0,3), and the unstable midpoint 1.5 V/1.5 V), so RustSpice's midpoint is a genuine solution of the equations, not a numerical error: what is judged is that .nodeset, the card that selects the state, is silently dropped (the mirrored deck cap_lang_nodeset_latch_low gets the same midpoint from RustSpice while ngspice flips to the other state). The low rail is a ~1.3e-9 V gmin-defined leakage value and IS comparable across the tools: when RustSpice is put in the stable state (a transient started with .ic v(q)=3 v(qb)=0 and run to 2 us) its low rail is 1.3043e-9 V against 1.3087e-9 V from tightly converged ngspice, q 2.9999999987 V in both; the default absolute tolerance (1e-9) is therefore enough and no tolerance override is needed (the former 1 uV override is removed, a tightening)
VDD vdd 0 3
MP1 q qb vdd vdd PM W=20u L=1u
MN1 q qb 0 0 NM W=10u L=1u
MP2 qb q vdd vdd PM W=20u L=1u
MN2 qb q 0 0 NM W=10u L=1u
.model NM NMOS(VTO=0.7 KP=100u LAMBDA=0.02)
.model PM PMOS(VTO=-0.7 KP=50u LAMBDA=0.02)
.nodeset v(q)=3 v(qb)=0
.op
.end
