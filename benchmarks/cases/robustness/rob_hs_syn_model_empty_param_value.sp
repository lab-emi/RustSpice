model parameter without a value (IS=)
* bench.id: rob_hs_syn_model_empty_param_value
* bench.group: robustness
* bench.title: model parameter without a value (IS=)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser, model, silent_risk
* bench.est_rss_mib: 64
* bench.note: v(2) = 0.6884150782 V for `D(IS=)` is identical to all digits to a bare `.model DM D`, and `D(IS= N=1.5)` equals `D(N=1.5)` (1.02466 V), so IS is dropped and the next parameter is not shifted. Valid `D(IS=1e-12)`: RustSpice = ngspice = 0.5709280059 V (independent mpmath solve 0.570928197 V). ngspice-47 stops with 'Syntax error: letter [)]' / 'Cannot compute substitute' (exit 1); `IS=abc` and `IS=nan` are dropped by RustSpice in the same way and rejected by ngspice [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
D9 2 0 DM
.model DM D(IS=)
.op
.end
