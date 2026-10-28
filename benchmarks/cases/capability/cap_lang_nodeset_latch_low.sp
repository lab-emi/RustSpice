.nodeset selects the other stable state of the latch
* bench.id: cap_lang_nodeset_latch_low
* bench.group: capability
* bench.title: same latch with .nodeset v(q)=0 v(qb)=3 must settle with q low
* bench.probes: v(q), v(qb)
* bench.tags: nonlinear, op, nodeset, bistable
* bench.note: the symmetric latch (beta_n = beta_p) has three DC solutions (two stable ones, (3,0) and (0,3), and the unstable midpoint 1.5 V/1.5 V), so RustSpice's midpoint is a genuine solution of the equations, not a numerical error: what is judged is that .nodeset, the card that selects the state, is silently dropped (the mirrored deck cap_lang_nodeset_latch_high gets the same midpoint from RustSpice while ngspice flips to the other state). The low rail is a ~1.3e-9 V gmin-defined leakage value and IS comparable across the tools: when RustSpice is put in the stable state (a transient started with .ic v(q)=0 v(qb)=3 and run to 2 us) its low rail is 1.3043e-9 V against 1.3087e-9 V from tightly converged ngspice, qb 2.9999999987 V in both, so the default absolute tolerance (1e-9) is enough and no tolerance override is needed (the former 1 uV override is removed, a tightening) [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
VDD vdd 0 3
MP1 q qb vdd vdd PM W=20u L=1u
MN1 q qb 0 0 NM W=10u L=1u
MP2 qb q vdd vdd PM W=20u L=1u
MN2 qb q 0 0 NM W=10u L=1u
.model NM NMOS(VTO=0.7 KP=100u LAMBDA=0.02)
.model PM PMOS(VTO=-0.7 KP=50u LAMBDA=0.02)
.nodeset v(q)=0 v(qb)=3
.op
.end
