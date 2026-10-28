5-stage CMOS ring oscillator started with .ic + uic
* bench.id: rob_cv_ring5_ic_uic_tran
* bench.group: robustness
* bench.title: 5-stage CMOS ring oscillator started with .ic + uic
* bench.probes: v(n1), v(n2), v(n3)
* bench.tol.norm: 2e-2
* bench.expect: match
* bench.tags: nonlinear, oscillator, ic, uic, tran
* bench.note: ngspice-47 skips the OP (uic) and oscillates; RustSpice computes the symmetric metastable OP first and Newton fails to converge for 5 and 7 stages (an OP-only 5-stage ring with different sizing converges, so the failure is sizing-dependent, not just stage-count). Calibration evidence: an independent 5-stage Radau ODE (Level-1 equations, rtol 1e-9) fed with the same .ic start agrees with tight ngspice to about 6e-6 of the swing, so the reference is trustworthy. The same deck as .op (no tran) converges in RustSpice with its homotopy, but the transient initial OP uses no homotopy and fails, even though uic asks for the OP to be skipped; the engine does not honour uic's skip. Slug linear-static-pivot-accuracy (read as covering nonlinear Newton): the initial-OP Newton stalls at the same 3.0314e-11 A residual as acc_tr_ring5, a round-off floor of the frozen static-pivot LU above the absolute abstol; faer LU alone and abstol=1e-9 alone each close it (reproduced, gap_closed under the faer LU). The ignored uic keyword (silent-ignore-ic) is a separate co-defect: with identical output with and without uic, RustSpice always solves the OP that ngspice skips.
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
VDD vdd 0 DC 3.3
MP1 n1 n5 vdd vdd PMOD W=20u L=1u
MN1 n1 n5 0 0 NMOD W=10u L=1u
CL1 n1 0 200f
MP2 n2 n1 vdd vdd PMOD W=20u L=1u
MN2 n2 n1 0 0 NMOD W=10u L=1u
CL2 n2 0 200f
MP3 n3 n2 vdd vdd PMOD W=20u L=1u
MN3 n3 n2 0 0 NMOD W=10u L=1u
CL3 n3 0 200f
MP4 n4 n3 vdd vdd PMOD W=20u L=1u
MN4 n4 n3 0 0 NMOD W=10u L=1u
CL4 n4 0 200f
MP5 n5 n4 vdd vdd PMOD W=20u L=1u
MN5 n5 n4 0 0 NMOD W=10u L=1u
CL5 n5 0 200f
.ic v(n1)=0 v(n2)=3.3 v(n3)=0 v(n4)=3.3 v(n5)=0
.tran 5p 6n uic
.end
