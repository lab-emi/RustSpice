Five-stage CMOS inverter chain, pulse input, 2000 steps
* bench.id: perf_nl_inv_chain5_tran_2e3
* bench.group: performance
* bench.title: Five-stage CMOS inverter chain, pulse input, 2000 steps
* bench.probes: v(n1), v(n3), v(n5)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 0.5p
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 9
* bench.est_rss_mib: 10
* bench.tags: nonlinear, tran, cmos, jit_tier, startup
* bench.note: ten MOSFETs, five 100 fF loads, one 20 ns input period in 2000 steps: short enough that any per-process JIT compile is a visible share; the supply current is not probed because its switching spikes cannot be resolved on the 10 ps comparison grid
.model nm NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.model pm PMOS(LEVEL=1 VTO=-0.7 KP=50u LAMBDA=0.02)
VDD vdd 0 3
VIN in0 0 PULSE(0 3 1n 1n 1n 8n 20n)
MP1 n1 in0 vdd vdd pm W=20u L=1u
MN1 n1 in0 0 0 nm W=10u L=1u
CL1 n1 0 100f
MP2 n2 n1 vdd vdd pm W=20u L=1u
MN2 n2 n1 0 0 nm W=10u L=1u
CL2 n2 0 100f
MP3 n3 n2 vdd vdd pm W=20u L=1u
MN3 n3 n2 0 0 nm W=10u L=1u
CL3 n3 0 100f
MP4 n4 n3 vdd vdd pm W=20u L=1u
MN4 n4 n3 0 0 nm W=10u L=1u
CL4 n4 0 100f
MP5 n5 n4 vdd vdd pm W=20u L=1u
MN5 n5 n4 0 0 nm W=10u L=1u
CL5 n5 0 100f
.tran 10p 20n
.end
