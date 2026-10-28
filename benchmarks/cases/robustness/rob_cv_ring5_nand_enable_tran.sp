5-stage NAND-enabled CMOS ring oscillator (deterministic OP, oscillation starts at 1 ns)
* bench.id: rob_cv_ring5_nand_enable_tran
* bench.group: robustness
* bench.title: 5-stage NAND-enabled CMOS ring oscillator (deterministic OP, oscillation starts at 1 ns)
* bench.probes: v(n1), v(n3)
* bench.tol.norm: 1.5e-1
* bench.expect: match
* bench.tags: nonlinear, oscillator, tran
* bench.note: Regression anchor with a well-conditioned OP (en=0 pins every node to a rail). Error vs the converged reference is 0.109 for RustSpice and 0.106 for ngspice as-is: both drift in phase (~0.8% period error), hence tol.norm 0.15.
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
VDD vdd 0 DC 3.3
VEN en 0 PULSE(0 3.3 1n 100p 100p 1 2)
MPA1 n1 en vdd vdd PMOD W=20u L=1u
MPB1 n1 n5 vdd vdd PMOD W=20u L=1u
MNA1 n1 en x1 0 NMOD W=20u L=1u
MNB1 x1 n5 0 0 NMOD W=20u L=1u
CL1 n1 0 20f
MP2 n2 n1 vdd vdd PMOD W=20u L=1u
MN2 n2 n1 0 0 NMOD W=10u L=1u
CL2 n2 0 20f
MP3 n3 n2 vdd vdd PMOD W=20u L=1u
MN3 n3 n2 0 0 NMOD W=10u L=1u
CL3 n3 0 20f
MP4 n4 n3 vdd vdd PMOD W=20u L=1u
MN4 n4 n3 0 0 NMOD W=10u L=1u
CL4 n4 0 20f
MP5 n5 n4 vdd vdd PMOD W=20u L=1u
MN5 n5 n4 0 0 NMOD W=10u L=1u
CL5 n5 0 20f
.tran 2p 3n
.end
