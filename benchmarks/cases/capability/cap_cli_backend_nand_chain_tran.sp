Three-stage CMOS NAND2 chain, pulse input: transient for the backend matrix
* bench.id: cap_cli_backend_nand_chain_tran
* bench.group: capability
* bench.title: Three-stage CMOS NAND2 chain with 300 fF loads and a 5 ns-edge pulse, transient backend-matrix fixture
* bench.probes: v(o1), v(o2), v(o3)
* bench.expect: match
* bench.tol.norm: 5e-3
* bench.floor: 0.1
* bench.est_rss_mib: 40
* bench.tags: mos, level1, tran, nonlinear, cmos, backend_fixture, cli_surface, npoints
* bench.note: fixture of checks/untested_cli_surface/check_backend_equiv.py (transient path of every backend); slow 5 ns input edges, 300 fF loads and 5 kohm loads to 1.65 V keep every output edge >= 1 ns so the fixed 0.05 ns grid resolves them. The deck itself is an ordinary transient comparison against ngspice.
V1 vdd 0 DC 3.3
V3 hi 0 DC 3.3
V4 vm 0 DC 1.65
V2 in 0 PULSE(0 3.3 5n 5n 5n 25n 60n)
MN1A o1 in m1 0 NN W=12u L=1u
MN1B m1 hi 0 0 NN W=12u L=1u
MP1A o1 in vdd vdd NP W=20u L=1u
MP1B o1 hi vdd vdd NP W=20u L=1u
RL1 o1 vm 5k
CL1 o1 0 300f
MN2A o2 o1 m2 0 NN W=12u L=1u
MN2B m2 hi 0 0 NN W=12u L=1u
MP2A o2 o1 vdd vdd NP W=20u L=1u
MP2B o2 hi vdd vdd NP W=20u L=1u
RL2 o2 vm 5k
CL2 o2 0 300f
MN3A o3 o2 m3 0 NN W=12u L=1u
MN3B m3 hi 0 0 NN W=12u L=1u
MP3A o3 o2 vdd vdd NP W=20u L=1u
MP3B o3 hi vdd vdd NP W=20u L=1u
RL3 o3 vm 5k
CL3 o3 0 300f
.model NN NMOS(LEVEL=1 VTO=0.7 KP=110u LAMBDA=0.03 GAMMA=0.4 PHI=0.7)
.model NP PMOS(LEVEL=1 VTO=-0.8 KP=38u LAMBDA=0.04 GAMMA=0.5 PHI=0.7)
.tran 0.05n 80n
.end
