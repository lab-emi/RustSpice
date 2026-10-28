VCCS cancels R at DC (singular OP) but the transient is well posed through C
* bench.id: rob_hs_num_tran_gm_cancel_at_op
* bench.group: robustness
* bench.title: VCCS cancels R at DC (singular OP) but the transient is well posed through C
* bench.probes: v(2)
* bench.expect: either
* bench.tags: caught_panic_noise, hostile, numeric, singular, tran
* bench.est_rss_mib: 64
* bench.note: at t=0 (v1=0) the node-2 equation is 0*v2 = 0: consistent but with infinitely many solutions, so the initial OP is genuinely undetermined and the transient (v2 = v2(0) + integral of v1/(R*C)) depends on a choice the netlist does not make. ngspice warns "singular matrix" and picks v2(0)=0, giving v(2)(5 ns) = 4.5000 V (= 1e9*(0.5n+4n)); RustSpice reports SingularMatrix. Refusing an undetermined OP is defensible and so is the gmin choice: ambiguous
V1 1 0 PULSE(0 1 0 1n 1n 1 2)
R1 1 2 1k
G1 2 0 2 0 -1m
C1 2 0 1p
.tran 1n 5n
.end
