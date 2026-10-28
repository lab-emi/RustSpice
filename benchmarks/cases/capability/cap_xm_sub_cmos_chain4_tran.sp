Hierarchical four-stage CMOS inverter chain built from one .subckt
* bench.id: cap_xm_sub_cmos_chain4_tran
* bench.group: capability
* bench.title: 4-stage CMOS inverter chain from a .subckt inv (20 fF load per stage), tstep 1 ps
* bench.probes: v(n1), v(n2), v(n3), v(n4), i(vdd)
* bench.tol.norm: 3e-3
* bench.expect: match
* bench.tags: subckt, mosfet, cmos, tran, pulse, npoints
* bench.est_rss_mib: 16
* bench.note: X x .tran with MOS devices inside a subcircuit (the corpus has one X .tran deck). Checked against an independent oracle (hand-written level-1 equations, no gate capacitance in this model, 4-state ODE, scipy Radau rtol 1e-10, restarted at the PULSE corners): converged ngspice within 3.1e-7..2.7e-6 of swing, RustSpice within 1.2e-4..6.9e-4 on v(n1)..v(n4) and 1.09e-3 on i(vdd) at 1 ps (ngspice as-is 1.2e-4..6.9e-4 and 1.10e-3). The accuracy limit is the step, for BOTH tools: at 5 ps v(n4) is 1.58e-2 (RustSpice) vs 1.50e-2 (ngspice as-is) and i(vdd) 2.5e-2 vs 2.7e-2 from the oracle, so the coarse step is not a RustSpice gap (SUITE_SPEC 13.4); RustSpice converges second order to the oracle (v(n4): 1.58e-2, 4.3e-3, 2.8e-3, 6.9e-4 at 5, 2.5, 2, 1 ps); 1 ps keeps the tolerance at 3e-3 instead of loosening it. Hand-flattened twin identical bit for bit in both tools (checks/element_analysis_matrix_holes/check_hierarchy_flat_equivalence.py).
.subckt inv in out vdd
MP1 out in vdd vdd PMOD W=20u L=1u
MN1 out in 0 0 NMOD W=10u L=1u
CL out 0 20f
.ends
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
VDD vdd 0 DC 3.3
VIN in1 0 PULSE(0.0 3.3 0.2n 0.1n 0.1n 2n 4n)
X1 in1 n1 vdd inv
X2 n1 n2 vdd inv
X3 n2 n3 vdd inv
X4 n3 n4 vdd inv
.tran 1p 3n
.end
