Transient stop time from a parameter expression
* bench.id: cap_lang_tran_stop_param
* bench.group: capability
* bench.title: .tran 10n {1u*n} (n = 5) takes its stop time from an expression starting with a number
* bench.probes: v(out)
* bench.known_gap: expr-truncation
* bench.gap.note: {1u*n} is truncated to 1u: RustSpice writes 101 rows ending at t = 1.0e-6 s (exit 0, no message) where ngspice runs to 5 us
* bench.tags: linear, tran, param
* bench.note: closed form: RC step response with tau = 1 us and the 1 ns ramp, v(out)(5 us) = 5*(1-exp(-(5us-0.5ns)/1us)) = 4.966293 V, which ngspice-47 reproduces (4.96629 V at 5 us with a literal 5u). `{n*1u}` is refused loudly and `{5u}` runs to 5 us. The probe comparison window is clipped to RustSpice's own 0..1 us run, where the probe is inside tolerance (worst error 2.2e-3 V against ngspice, 7.1e-4 of the swing, tol.norm 5e-3), so the violation is detected by the end-time check only; the missing 1..5 us never enters the probe error.
.param n=5
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 10n {1u*n}
.end
