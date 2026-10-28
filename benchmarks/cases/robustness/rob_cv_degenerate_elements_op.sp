Degenerate elements: chained zero-volt sense sources and R, C, D, I and MOS with all terminals on one node
* bench.id: rob_cv_degenerate_elements_op
* bench.group: robustness
* bench.title: Degenerate elements: chained zero-volt sense sources and R, C, D, I and MOS with all terminals on one node
* bench.probes: v(1), v(2), v(3), v(4), i(vm1), i(vm2)
* bench.expect: match
* bench.tags: linear, degenerate, shorted_elements, op
* bench.note: Regression anchor: ngspice-47 and RustSpice both treat zero-volt sources as wires (sense currents 2.5 mA) and self-connected elements as no-ops; all probes agree exactly (the exact answers are v(4)=2.5 V, i(vm1)=i(vm2)=2.5 mA).
V1 1 0 5
Vm1 1 2 0
Vm2 2 3 0
R1 3 4 1k
R2 4 0 1k
R3 4 4 1k
C1 4 4 1n
D1 4 4 dm
I1 4 4 1m
M1 4 4 4 4 nm w=10u l=1u
.model dm D(is=1e-14 n=1)
.model nm nmos(level=1 vto=0.7 kp=100u)
.op
.end
