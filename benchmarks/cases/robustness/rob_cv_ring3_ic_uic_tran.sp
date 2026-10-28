3-stage CMOS ring oscillator started with .ic + uic (no DC operating point wanted)
* bench.id: rob_cv_ring3_ic_uic_tran
* bench.group: robustness
* bench.title: 3-stage CMOS ring oscillator started with .ic + uic (no DC operating point wanted)
* bench.probes: v(n1), v(n2), v(n3)
* bench.tol.norm: 2e-2
* bench.expect: match
* bench.tags: nonlinear, oscillator, ic, uic, tran
* bench.note: Regression anchor: RustSpice ignores uic but applies .ic after its own OP and matches ngspice (2.6e-3, as good as ngspice as-is). Oscillators accumulate phase error, hence the 6 ns window (~4 periods) and the slow 200 fF ring; ngspice as-is is within 2e-3 of the converged reference so tol.norm 2e-2 is generous.
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
VDD vdd 0 DC 3.3
MP1 n1 n3 vdd vdd PMOD W=20u L=1u
MN1 n1 n3 0 0 NMOD W=10u L=1u
CL1 n1 0 200f
MP2 n2 n1 vdd vdd PMOD W=20u L=1u
MN2 n2 n1 0 0 NMOD W=10u L=1u
CL2 n2 0 200f
MP3 n3 n2 vdd vdd PMOD W=20u L=1u
MN3 n3 n2 0 0 NMOD W=10u L=1u
CL3 n3 0 200f
.ic v(n1)=0 v(n2)=3.3 v(n3)=0
.tran 5p 6n uic
.end
