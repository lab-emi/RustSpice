7-stage CMOS ring oscillator started with .ic + uic
* bench.id: rob_cv_ring7_ic_uic_tran
* bench.group: robustness
* bench.title: 7-stage CMOS ring oscillator started with .ic + uic
* bench.probes: v(n1), v(n4), v(n7)
* bench.tol.norm: 2e-2
* bench.expect: match
* bench.tags: nonlinear, oscillator, ic, uic, tran
* bench.note: Same as the 5-stage case with a longer ring (period grows with stage count, window 8 ns). ngspice-47 skips the OP via uic. Calibration evidence: an independent 7-stage Radau ODE with the same .ic start agrees with tight ngspice to about 1.6e-4 of the swing (well below the 2e-2 tolerance). Unlike the 5-stage ring, the 7-stage .op also fails in RustSpice even with its homotopy, so the uic-ignored initial OP is not the only obstacle here. Slug linear-static-pivot-accuracy (read as covering nonlinear Newton): the faer LU alone closes the deck (reproduced, gap_closed), abstol=1e-9 alone does not, and a homotopy fallback in the transient initial solve does not either (second-look), so no-op-homotopy-tran does not describe it; the ignored uic keyword (silent-ignore-ic, identical output with and without uic) is a co-cause.
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
VDD vdd 0 DC 3.3
MP1 n1 n7 vdd vdd PMOD W=20u L=1u
MN1 n1 n7 0 0 NMOD W=10u L=1u
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
MP6 n6 n5 vdd vdd PMOD W=20u L=1u
MN6 n6 n5 0 0 NMOD W=10u L=1u
CL6 n6 0 200f
MP7 n7 n6 vdd vdd PMOD W=20u L=1u
MN7 n7 n6 0 0 NMOD W=10u L=1u
CL7 n7 0 200f
.ic v(n1)=0 v(n2)=3.3 v(n3)=0 v(n4)=3.3 v(n5)=0 v(n6)=3.3 v(n7)=0
.tran 5p 8n uic
.end
