Node between two reverse-biased diodes: voltage set only by IS vs gmin
* bench.id: rob_cv_diode_gmin_midnode_op
* bench.group: robustness
* bench.title: Node between two reverse-biased diodes: voltage set only by IS vs gmin
* bench.probes: v(2)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, diode, gmin_sensitive, op
* bench.note: Regression anchor for gmin sensitivity: v(2)=0.1591493 V is fixed by the 1e-12 S junction conductances; RustSpice matches ngspice-tight to 5e-11 V (default gmin 1e-12 in both).
V1 1 0 5
D1 2 1 dm
D2 2 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
