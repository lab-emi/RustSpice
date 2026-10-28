Diode between +5 V and a -5 V rail made by a reversed source
* bench.id: acc_dc_clamp_negative_rail_diode
* bench.group: accuracy
* bench.title: Diode between +5 V and a -5 V rail made by a reversed source
* bench.probes: v(2), v(3), i(v1), i(v2)
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, negative_rail
* bench.note: Reference verified: tight ngspice (v(2) = -5 V, v(3) = -4.28723848 V, i(v1) = i(v2) = -9.2872385e-3 A) agrees with an independent 50-digit mpmath solve of the Shockley model (Vt = 0.025864917007155805, gmin = 1e-12) to 1e-14 relative. With the node-voltage box disabled in a private experimental build the deck is gap_closed.
V1 1 0 5
V2 0 2 5
R1 1 3 1k
D1 3 2 dm
.model dm D(IS=1e-14 N=1)
.op
.end
