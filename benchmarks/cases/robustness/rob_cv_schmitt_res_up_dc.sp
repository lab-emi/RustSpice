Resistive-feedback Schmitt trigger (two CMOS inverters + 10k/40k), input swept up 0 -> 3 V
* bench.id: rob_cv_schmitt_res_up_dc
* bench.group: robustness
* bench.title: Resistive-feedback Schmitt trigger (two CMOS inverters + 10k/40k), input swept up 0 -> 3 V
* bench.probes: v(out)
* bench.expect: match
* bench.tags: nonlinear, bistable, hysteresis, positive_feedback, dc
* bench.note: ngspice-47 sweeps the full range with hysteresis. Reference verified: every ngspice-tight point is an exact root of an independent dense f64 KCL system (deviation 1.2e-8 V from gmin placement, residual 5e-19 A); at VIN=1.85 V three roots exist (0.0222, 0.0825, 2.990 V) and at 1.875 V only the high one, so the low branch folds between 1.85 and 1.875 V where ngspice jumps. Mechanism check: RustSpice's .dc over 0..1.85 V works, but a .dc consisting of the single point 1.9 V fails while a .op at VIN=1.9 V converges to the same 2.9904 V (the .op path has gmin/source-stepping, the sweep path has none). Flips (reproduced): the faer LU alone, and dummy +-3, 5, 10, 40 and 300 V source pairs (a wider clamp box than [0, 3 V]) alone, each make the sweep converge and match tight ngspice to 4e-11 V, abstol=1e-9 does not: the missing per-point recovery named by the slug is one of several independent single flips (second-look: a per-point homotopy recovery also closes it), so it is sufficient but not the only cause; slug kept.
VDD vdd 0 3
R1 in x 10k
RF out x 40k
MP1 m x vdd vdd PM W=20u L=1u
MN1 m x 0 0 NM W=10u L=1u
MP2 out m vdd vdd PM W=20u L=1u
MN2 out m 0 0 NM W=10u L=1u
.model NM NMOS(VTO=0.7 KP=100u LAMBDA=0.02)
.model PM PMOS(VTO=-0.7 KP=50u LAMBDA=0.02)
VIN in 0 0
.dc VIN 0 3 0.05
.end
