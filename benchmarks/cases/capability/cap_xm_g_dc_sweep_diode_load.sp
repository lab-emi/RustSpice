VCCS driving a resistor in parallel with a diode, control voltage swept -1 to 1 V
* bench.id: cap_xm_g_dc_sweep_diode_load
* bench.group: capability
* bench.title: VCCS (2 mS) from a divider-controlled node into 1k || diode, V1 swept -1..1 V
* bench.probes: v(c), v(out), i(v1)
* bench.expect: match
* bench.tol.rel: 1e-8
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear
* bench.tags: controlled, vccs, dc, sweep, diode
* bench.est_rss_mib: 16
* bench.note: Fills the empty G x .dc cell (the corpus had 11 G .op, 2 G .tran, 0 G .dc decks). The control port draws no current (v(c) = V1/2 exactly, i(v1) = -V1/2k, +-0.5 mA) and the controlled current gm*v(c) flows into out: v(out) runs from -1.0 V (reverse diode, resistor only) to the diode clamp 0.6294 V. Independent mpmath oracle and engine oracle dc_nonlinear (status ok): RustSpice 1.7e-10 V, tight ngspice 2.9e-10 V from the oracle, ngspice as-is 4.8e-4 V. Worst probe uses 4.3 % of the band.
V1 in 0 0
R0 in c 1k
R3 c 0 1k
G1 0 out c 0 2m
R1 out 0 1k
D1 out 0 dm
.model dm D(IS=1e-14 N=1)
.dc V1 -1 1 0.1
.end
