Capacitive divider (C1 = C2 = 1 nF) with a 1 Pohm bleed so that DC is non-singular, pulse input
* bench.id: rob_cv_sing_cap_divider_bleed_tran
* bench.group: robustness
* bench.title: Capacitive divider (C1 = C2 = 1 nF) with a 1 Pohm bleed so that DC is non-singular, pulse input
* bench.probes: v(1), v(2)
* bench.tol.norm: 1e-3
* bench.expect: match
* bench.tags: linear, floating, tran
* bench.note: Regression anchor: v(2) follows 2.5 V exactly as in ngspice-47. Without Rb (pure cap-only node) ngspice warns at the OP but the transient is identical (RustSpice v(2) matched to 6e-11 V).
V1 1 0 PULSE(0 5 1u 1n 1n 10u 20u)
C1 1 2 1n
C2 2 0 1n
R1 1 0 1k
Rb 2 0 1e15
.tran 100n 10u
.end
