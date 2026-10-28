Current pulse into an R-C node with a diode branch through a series resistor
* bench.id: cap_xm_i_pulse_diode_rc_tran
* bench.group: capability
* bench.title: PULSE current source (-0.5 mA .. +1 mA, edges on the 1 ns grid) into 2k || 50 pF plus a 500 ohm + diode branch
* bench.probes: v(a), v(b)
* bench.tol.norm: 2e-4
* bench.expect: match
* bench.tags: source, isrc, tran, diode, pulse, npoints
* bench.est_rss_mib: 16
* bench.note: I x .tran x diode: the diode is reverse biased at the -0.5 mA base level (v(a) = v(b) = -1.0 V through R2, the initial operating point) and conducts at the 1 mA plateau (v(a) up to 0.6806 V, v(b) 0.6059 V). Edges (delay 20 ns, 5 ns rise/fall) are multiples of the 1 ns step on purpose: unaligned edges are the registered fixed-step-accuracy gap covered by cap_el_v_pulse_unaligned* and acc_tr_rc_pulse_offgrid. Independent oracle (hand-derived node equations, inner root find for the diode node, scipy Radau rtol 1e-12, restarted at the pulse corners): converged ngspice 7.6e-8 / 4.8e-8 of swing from it, RustSpice 4.0e-5 / 1.5e-5, ngspice as-is 2.1e-5 / 2.6e-5 (same size); tol.norm 2e-4 is 5x the RustSpice error and 40x below the default.
I1 0 a PULSE(-0.5m 1m 20n 5n 5n 80n 200n)
R1 a 0 2k
C1 a 0 50p
R2 a b 500
D1 b 0 dm
.model dm D(IS=1e-14 N=1)
.tran 1n 200n
.end
