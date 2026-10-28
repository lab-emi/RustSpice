5-stage CMOS ring oscillator with a start-up current kick
* bench.id: acc_tr_ring5
* bench.group: accuracy
* bench.title: 5-stage CMOS ring oscillator (100 fF per stage) kicked by a 0.1 ns current pulse, tstep 2 ps
* bench.probes: v(n1), v(n2)
* bench.tol.norm: 2e-2
* bench.expect: match
* bench.size: 5
* bench.tags: nonlinear, tran, mos, cmos, oscillator
* bench.est_rss_mib: 16
* bench.note: Slug linear-static-pivot-accuracy (read as covering nonlinear Newton: the registry text only names linear nets): the residual stalls at a round-off floor of the frozen static-pivot LU against the absolute 1e-12 A criterion; faer LU alone and abstol=1e-9 alone each close it (reproduced with a private env-gated build, gap_closed under the faer LU). A homotopy fallback in the transient initial solve is a second sufficient flip, and the plain Newton of the .op path stalls the same way before its homotopy rescues it (both second-look, not reproduced here); the earlier tag no-op-homotopy-tran named that .op/.tran asymmetry.
VDD vdd 0 DC 3.3
MP1 n1 n5 vdd vdd PMOD W=20u L=1u
MN1 n1 n5 0 0 NMOD W=10u L=1u
CL1 n1 0 100f
MP2 n2 n1 vdd vdd PMOD W=20u L=1u
MN2 n2 n1 0 0 NMOD W=10u L=1u
CL2 n2 0 100f
MP3 n3 n2 vdd vdd PMOD W=20u L=1u
MN3 n3 n2 0 0 NMOD W=10u L=1u
CL3 n3 0 100f
MP4 n4 n3 vdd vdd PMOD W=20u L=1u
MN4 n4 n3 0 0 NMOD W=10u L=1u
CL4 n4 0 100f
MP5 n5 n4 vdd vdd PMOD W=20u L=1u
MN5 n5 n4 0 0 NMOD W=10u L=1u
CL5 n5 0 100f
IK 0 n1 PULSE(0 1m 0.1n 20p 20p 100p 1)
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 2p 4n
.end
