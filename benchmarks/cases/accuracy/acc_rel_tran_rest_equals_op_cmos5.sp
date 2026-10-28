Three DC-driven 5-stage CMOS inverter chains (Vin 1.5 V, 1.65 V, 1.8 V): the transient must stay at the operating point
* bench.id: acc_rel_tran_rest_equals_op_cmos5
* bench.group: accuracy
* bench.title: Rest state: .tran of DC-driven CMOS chains stays at the .op values (Vin 1.5 V / 1.65 V trip point / 1.8 V, 20 fF per stage)
* bench.probes: v(a1), v(a3), v(a5), v(b1), v(b2), v(b3), v(c1), v(c2), v(c4)
* bench.tol.norm: 1
* bench.floor: 1e-8
* bench.expect: match
* bench.size: 24
* bench.tags: nonlinear, tran, mos, cmos, rest_state, reference_free
* bench.est_rss_mib: 16
* bench.note: Chains a, b, c are DC-driven at 1.5 V, 1.65 V (trip point of the symmetric inverter) and 1.8 V from a 3.3 V supply; the exact solution of .tran is the constant operating point. ngspice's flat waveform (tight reference) is the reference; floor 1e-8 V x tol.norm 1 makes the gate |v_rs(t) - v_ng(t)| <= 1e-8 V. Measured RustSpice deviation: <= 5.8e-15 V on a1, a3, a5, c1, c2, c4 and 5.7e-14 / 4.9e-12 / 3.7e-10 V on b1 / b2 / b3 (gate margin 27x on b3). Not probed: a2, a4, c3, c5 sit at ~1e-9 V (vacuous) and b4, b5, because the stage gain is ~100 at the trip point: the RustSpice operating point differs from tight ngspice by 9.8e-15, 1.0e-12, 1.1e-10, 1.2e-8, 1.3e-6 V on b1..b5 (Newton converged to 1e-6 relative) and the transient inherits it. The reference-free statement over all 18 nodes (tran vs RustSpice's own .op, max 6.0e-7 V at b5, gate reltol 1e-6) is checks/reference_free_relations/check_tran_rest_equals_op.py.
vdd vdd 0 DC 3.3
vina a0 0 DC 1.5
mpa1 a1 a0 vdd vdd PMOD W=20u L=1u
mna1 a1 a0 0 0 NMOD W=10u L=1u
cla1 a1 0 20f
mpa2 a2 a1 vdd vdd PMOD W=20u L=1u
mna2 a2 a1 0 0 NMOD W=10u L=1u
cla2 a2 0 20f
mpa3 a3 a2 vdd vdd PMOD W=20u L=1u
mna3 a3 a2 0 0 NMOD W=10u L=1u
cla3 a3 0 20f
mpa4 a4 a3 vdd vdd PMOD W=20u L=1u
mna4 a4 a3 0 0 NMOD W=10u L=1u
cla4 a4 0 20f
mpa5 a5 a4 vdd vdd PMOD W=20u L=1u
mna5 a5 a4 0 0 NMOD W=10u L=1u
cla5 a5 0 20f
vinb b0 0 DC 1.65
mpb1 b1 b0 vdd vdd PMOD W=20u L=1u
mnb1 b1 b0 0 0 NMOD W=10u L=1u
clb1 b1 0 20f
mpb2 b2 b1 vdd vdd PMOD W=20u L=1u
mnb2 b2 b1 0 0 NMOD W=10u L=1u
clb2 b2 0 20f
mpb3 b3 b2 vdd vdd PMOD W=20u L=1u
mnb3 b3 b2 0 0 NMOD W=10u L=1u
clb3 b3 0 20f
mpb4 b4 b3 vdd vdd PMOD W=20u L=1u
mnb4 b4 b3 0 0 NMOD W=10u L=1u
clb4 b4 0 20f
mpb5 b5 b4 vdd vdd PMOD W=20u L=1u
mnb5 b5 b4 0 0 NMOD W=10u L=1u
clb5 b5 0 20f
vinc c0 0 DC 1.8
mpc1 c1 c0 vdd vdd PMOD W=20u L=1u
mnc1 c1 c0 0 0 NMOD W=10u L=1u
clc1 c1 0 20f
mpc2 c2 c1 vdd vdd PMOD W=20u L=1u
mnc2 c2 c1 0 0 NMOD W=10u L=1u
clc2 c2 0 20f
mpc3 c3 c2 vdd vdd PMOD W=20u L=1u
mnc3 c3 c2 0 0 NMOD W=10u L=1u
clc3 c3 0 20f
mpc4 c4 c3 vdd vdd PMOD W=20u L=1u
mnc4 c4 c3 0 0 NMOD W=10u L=1u
clc4 c4 0 20f
mpc5 c5 c4 vdd vdd PMOD W=20u L=1u
mnc5 c5 c4 0 0 NMOD W=10u L=1u
clc5 c5 0 20f
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 10p 2n
.end
