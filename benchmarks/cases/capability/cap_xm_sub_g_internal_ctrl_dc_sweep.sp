VCCS stages inside a subcircuit controlled by an internal divider node, cascaded three times
* bench.id: cap_xm_sub_g_internal_ctrl_dc_sweep
* bench.group: capability
* bench.title: .subckt gst (divider + VCCS controlled by its internal node) x3 with a diode clamp at the output, V1 swept -0.3..0.3 V
* bench.probes: v(x1.m), v(n1), v(n2), v(n3), i(v1)
* bench.expect: match
* bench.tol.rel: 1e-8
* bench.tol.abs: 1e-12
* bench.tags: subckt, controlled, vccs, dc, sweep, diode
* bench.est_rss_mib: 16
* bench.note: No corpus subckt contains a G, L or I element. The VCCS control nodes here are an internal node (m, probed as x1.m) and ground, so the flattening has to rename the CONTROLLING terminals as well as the output pair. Stages 1-2 are linear (v(n1) +-0.40 V, v(n2) +-0.533 V at V1 = +-0.3 V, errors 1e-16), stage 3 ends in the diode clamp with a top-level 2k load R4 (v(n3) from -0.7111 V to +0.6071 V, diode knee between V1 = 0.2 V and 0.3 V). Hand-flattened twin identical bit for bit in both tools (checks/element_analysis_matrix_holes/check_hierarchy_flat_equivalence.py); independent mpmath oracle: RustSpice 4.2e-11 V, tight ngspice 1.1e-10 V, ngspice as-is 1.8e-4 V from it (checks/element_analysis_matrix_holes/check_independent_oracles.py); the top-level R4 gives the engine teeth check something to scale. Worst probe uses 2.7 % of the band.
.subckt gst in out
R1 in m 1k
R2 m 0 1k
G1 0 out m 0 4m
R3 out 0 1k
.ends
.model dm D(IS=1e-14 N=1)
V1 in 0 0
X1 in n1 gst
X2 n1 n2 gst
X3 n2 n3 gst
D1 n3 0 dm
R4 n3 0 2k
.dc V1 -0.3 0.3 0.025
.end
