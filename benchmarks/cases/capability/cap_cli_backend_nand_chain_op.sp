Three-stage CMOS NAND2 chain at its switching threshold: operating point for the backend matrix
* bench.id: cap_cli_backend_nand_chain_op
* bench.group: capability
* bench.title: Three-stage CMOS NAND2 chain (12 level-1 MOSFETs) with the first input at its switching threshold, backend-matrix fixture
* bench.probes: v(o1), v(o2), v(o3), v(m1), i(v1)
* bench.expect: match
* bench.tol.rel: 1e-7
* bench.tol.abs: 1e-11
* bench.est_rss_mib: 40
* bench.tags: mos, level1, op, nonlinear, cmos, backend_fixture, cli_surface
* bench.note: fixture of checks/untested_cli_surface/check_backend_equiv.py (run --backend cpu|metal|cuda-tile|cuda-simt|wasm); the first stage sits in its high-gain region so a device-evaluation difference between backends would be amplified, each output has a 5 kohm load to a 1.65 V reference so the later stages stay off the rails and every device matters. The deck itself is an ordinary OP comparison against ngspice.
V1 vdd 0 DC 3.3
V3 hi 0 DC 3.3
V4 vm 0 DC 1.65
V2 in 0 DC 1.52
MN1A o1 in m1 0 NN W=12u L=1u
MN1B m1 hi 0 0 NN W=12u L=1u
MP1A o1 in vdd vdd NP W=20u L=1u
MP1B o1 hi vdd vdd NP W=20u L=1u
RL1 o1 vm 5k
MN2A o2 o1 m2 0 NN W=12u L=1u
MN2B m2 hi 0 0 NN W=12u L=1u
MP2A o2 o1 vdd vdd NP W=20u L=1u
MP2B o2 hi vdd vdd NP W=20u L=1u
RL2 o2 vm 5k
MN3A o3 o2 m3 0 NN W=12u L=1u
MN3B m3 hi 0 0 NN W=12u L=1u
MP3A o3 o2 vdd vdd NP W=20u L=1u
MP3B o3 hi vdd vdd NP W=20u L=1u
RL3 o3 vm 5k
.model NN NMOS(LEVEL=1 VTO=0.7 KP=110u LAMBDA=0.03 GAMMA=0.4 PHI=0.7)
.model NP PMOS(LEVEL=1 VTO=-0.8 KP=38u LAMBDA=0.04 GAMMA=0.5 PHI=0.7)
.op
.end
