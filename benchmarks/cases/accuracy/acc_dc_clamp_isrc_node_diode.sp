Current-driven 100 V node (1 mA into 100k) plus an unrelated diode branch
* bench.id: acc_dc_clamp_isrc_node_diode
* bench.group: accuracy
* bench.title: Current-driven 100 V node (1 mA into 100k) plus an unrelated diode branch
* bench.probes: v(1), v(3), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, isrc, current_driven_node
* bench.note: Reference verified: tight ngspice (v(1) = 100 V, v(3) = 0.62944071 V, i(v1) = -3.7056e-4 A) agrees with an independent 50-digit mpmath solve of the Shockley model (Vt = 0.025864917007155805, gmin = 1e-12) to 1e-13 relative. The diode branch only makes the circuit nonlinear (a purely linear deck takes the linear fast path). With the node-voltage box disabled in a private experimental build the deck is gap_closed, so the box is the whole cause.
I1 0 1 1m
R1 1 0 100k
V1 2 0 1
R2 2 3 1k
D1 3 0 dm
.model dm D(IS=1e-14 N=1)
.op
.end
