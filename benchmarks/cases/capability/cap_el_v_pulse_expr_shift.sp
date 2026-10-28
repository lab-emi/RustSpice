PULSE argument {vh*2} (parameter first) shifts the argument list
* bench.id: cap_el_v_pulse_expr_shift
* bench.group: capability
* bench.title: PULSE argument {vh*2} (parameter first) shifts the argument list
* bench.probes: v(in), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.tags: source, tran, waveform, pulse, param, expression, silent_risk
* bench.note: ngspice evaluates the expression: pulse amplitude 5 V (matches the closed form to 1e-14 V). RustSpice drops the unparsable {vh*2}, so v2 becomes the delay value 1u and the pulse is 1 uV high (max v(in) = 1e-6 V); the shifted tail reads td = 100n, tr = 100n, tf = 1u, pw = 3u. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
.param vh=2.5
V1 in 0 PULSE(0 {vh*2} 1u 100n 100n 1u 3u)
R1 in 0 1k
.tran 10n 6u
.end
