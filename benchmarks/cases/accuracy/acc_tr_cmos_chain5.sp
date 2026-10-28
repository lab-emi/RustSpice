5-stage CMOS inverter chain
* bench.id: acc_tr_cmos_chain5
* bench.group: accuracy
* bench.title: 5-stage CMOS inverter chain (20 fF per stage, 0.1 ns input edge), tstep 5 ps
* bench.probes: v(n1), v(n3), v(n5)
* bench.tol.norm: 3.5e-2
* bench.series.tstep: 20p 5p 2.5p 1.25p 0.625p
* bench.size: 5
* bench.tags: nonlinear, tran, mos, cmos, series
* bench.est_rss_mib: 16
* bench.note: Series gap: only the 20 ps member (coarser than the nominal 5 ps) is known_gap_confirmed, the finer members pass (the engine keeps the gap while any member confirms it). The reference is the converged ngspice and RustSpice's own refinement converges to it. At 5 ps both tools are ~2 % off the converged waveform (ngspice default 2.7e-2, RustSpice 1.9e-2); the work-precision series shows the convergence of both. The former 10 ps member (RustSpice 8.3e-2 on v(n5), ngspice as-is 4.4e-2, both above tol.norm, RustSpice only 1.9x worse) was judged ambiguous by the adversarial re-verification and moved out of the series into the information-only deck acc_tr_cmos_chain5_h10p, because a series shares one known_gap for all members.
VDD vdd 0 DC 3.3
VIN in1 0 PULSE(0.0 3.3 0.2n 0.1n 0.1n 2n 4n)
MP1 n1 in1 vdd vdd PMOD W=20u L=1u
MN1 n1 in1 0 0 NMOD W=10u L=1u
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
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 5p 2n
.end
