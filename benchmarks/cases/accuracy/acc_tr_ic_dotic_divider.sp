Initial condition forced on a divider node
* bench.id: acc_tr_ic_dotic_divider
* bench.group: accuracy
* bench.title: .ic v(b)=2 on a resistor-divider tail (V1 = 0 V, 1 k / 1 k into C1 = 1 uF), no UIC: the node between the resistors must start consistent with the forced node
* bench.probes: v(a), v(b)
* bench.tol.norm: 5e-3
* bench.known_gap: silent-ignore-ic
* bench.gap.note: .ic is overlaid after the operating point (misapplied, not ignored): only the t = 0 row is wrong, v(a) = 0 V instead of the consistent 1 V and i(v1) = 0 instead of 1 mA; that one of 151 rows gives the 1.29 norm error of v(a) (the other 150 rows agree with the closed form to 6.3e-5 of swing); v(b) agrees
* bench.tags: linear, tran, ic
* bench.est_rss_mib: 16
* bench.note: closed form v(b) = 2 exp(-t/2 ms), v(a) = v(b)/2, so v(a)(0) = 1 V and i(v1)(0) = 1 mA (KCL); tightly converged ngspice reproduces it (row 0: v(a) = 1 V, i(v1) = 1 mA; v(a) error 9.6e-6 of swing). The max-norm status is dominated by that single row (a property of the max-norm metric), which is the whole defect: the solution at t = 0 is not the DC solution consistent with the forced node, it is the DC OP with .ic patched in afterwards
V1 in 0 DC 0
R1 in a 1k
R2 a b 1k
C1 b 0 1u
.ic v(b)=2
.tran 20u 3m
.end
