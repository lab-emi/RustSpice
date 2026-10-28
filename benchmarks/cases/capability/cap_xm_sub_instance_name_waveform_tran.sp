A SIN current source inside a subcircuit whose instance name contains the letters PULSE
* bench.id: cap_xm_sub_instance_name_waveform_tran
* bench.group: capability
* bench.title: SIN current source in a .subckt instantiated as Xpulse1 (control: the same .subckt as X2), 1k load
* bench.probes: v(o1), v(o2)
* bench.expect: match
* bench.tags: subckt, source, waveform, parser, sine, tran, silent_risk, npoints
* bench.est_rss_mib: 16
* bench.note: Hierarchy-induced variant of cap_el_v_name_contains_pulse (there the user writes the keyword into the source name; here the source card is clean and only the instance name, copied into the flattened element name, triggers it). Closed form: v(o) = 1k * 1m * sin(2 pi 10 MHz t), algebraic; ngspice (converged reference) matches it to 9e-16, v(o2) is the control and passes. Mechanism proven by checks/element_analysis_matrix_holes/check_instname_trigger.py on 9 variants: RustSpice is wrong exactly for SIN sources inside instances named Xpulse1 / XPULSE_gen / Xmypulsegen, right for X1 / Xpuls1 / Xsin1, and right for a PULSE source inside Xsin1 / Xpulse1 (PULSE is searched first); ngspice is right on all. The hand-flattened twin with plain names matches ngspice (checks/element_analysis_matrix_holes/check_hierarchy_flat_equivalence.py). [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
.subckt gen out
I1 0 out SIN(0 1m 10meg 0)
R1 out 0 1k
.ends
Xpulse1 o1 gen
X2 o2 gen
.tran 1n 200n
.end
