Stacked sources 5 V + 5 V (node at 10 V) feeding a diode through 1k
* bench.id: acc_dc_clamp_stacked_vsrc_diode
* bench.group: accuracy
* bench.title: Stacked sources 5 V + 5 V (node at 10 V) feeding a diode through 1k
* bench.probes: v(2), v(3), i(v1), i(v2)
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, stacked_source
* bench.note: Reference verified: tight ngspice (v(2) = 10 V, v(3) = 0.71276152 V, i(v1) = i(v2) = -9.2872385e-3 A) agrees with an independent 50-digit mpmath solve of the Shockley model (Vt = 0.025864917007155805, gmin = 1e-12) to 6e-14 relative. With the node-voltage box disabled in a private experimental build the deck is gap_closed.
V1 1 0 5
V2 2 1 5
R1 2 3 1k
D1 3 0 dm
.model dm D(IS=1e-14 N=1)
.op
.end
