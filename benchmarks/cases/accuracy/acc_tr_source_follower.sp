NMOS source follower driving a load capacitor
* bench.id: acc_tr_source_follower
* bench.group: accuracy
* bench.title: NMOS source follower (W/L 20u/1u, 10 k source resistor, 2 pF load) driven by a 5 MHz sine around 2 V
* bench.probes: v(s)
* bench.tol.norm: 2e-3
* bench.tags: nonlinear, tran, mos, follower
* bench.note: tstep 0.5 ns (400 steps per period): RustSpice 6.0e-4, ngspice as-is 1.0e-4 of swing, 2nd-order convergence (4.3e-3 at 2 ns, 1.8e-3 at 1 ns, 1.7e-4 at 0.25 ns). Was 2 ns with tol 1e-2: a 5 % change of the follower W or of CL stayed inside that tolerance (mutation check).
* bench.est_rss_mib: 16
VDD vdd 0 DC 3.3
VIN g 0 SIN(2 0.8 5Meg)
M1 vdd g s 0 NMOD W=20u L=1u
RS s 0 10k
CL s 0 2p
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 0.5n 1u
.end
