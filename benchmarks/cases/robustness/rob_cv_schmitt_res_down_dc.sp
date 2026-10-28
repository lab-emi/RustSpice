Resistive-feedback Schmitt trigger, input swept down 3 -> 0 V
* bench.id: rob_cv_schmitt_res_down_dc
* bench.group: robustness
* bench.title: Resistive-feedback Schmitt trigger, input swept down 3 -> 0 V
* bench.probes: v(out)
* bench.expect: match
* bench.tags: nonlinear, bistable, hysteresis, positive_feedback, dc
* bench.note: Opposite branch of the hysteresis loop; ngspice-47 completes the sweep. Reference verified like the up-sweep: every ngspice-tight point is an exact root of an independent dense f64 KCL system (deviation 1.2e-8 V, residual 5e-19 A) and agrees with dense continuation to 3.3e-9 V. Mechanism check: a .dc consisting of the single point 1.1 V fails while a .op at VIN=1.1 V converges to 0.0096 V (the ngspice value is 0.010 V), so the sweep path lacks the recovery the .op path has. Flips (reproduced): dummy +-3, 5, 40 and 300 V source pairs (a wider clamp box than [0, 3 V]) alone make the sweep converge and match tight ngspice to 4.4e-12 V (+-10 V does not: chaotic), so clamp-rail is an equally sufficient flip, while the faer LU and abstol=1e-9 do not help; the slug is kept because .op at the failing point converges and the sweep path has no recovery.
VDD vdd 0 3
R1 in x 10k
RF out x 40k
MP1 m x vdd vdd PM W=20u L=1u
MN1 m x 0 0 NM W=10u L=1u
MP2 out m vdd vdd PM W=20u L=1u
MN2 out m 0 0 NM W=10u L=1u
.model NM NMOS(VTO=0.7 KP=100u LAMBDA=0.02)
.model PM PMOS(VTO=-0.7 KP=50u LAMBDA=0.02)
VIN in 0 3
.dc VIN 3 0 -0.05
.end
