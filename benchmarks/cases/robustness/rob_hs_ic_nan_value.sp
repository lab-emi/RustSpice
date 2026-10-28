ic v(2)=nan
* bench.id: rob_hs_ic_nan_value
* bench.group: robustness
* bench.title: .ic v(2)=nan
* bench.probes: v(2)
* bench.expect: loud_error
* bench.tags: hostile, ic, tran, nonfinite, silent_risk
* bench.est_rss_mib: 64
* bench.note: the registered silent-ignore-ic slug covers valid ic features that are ignored; here the card is malformed and silently discarded. ngspice-47 stops with 'Undefined parameter [nan]' / 'Cannot compute substitute' (exit 1). Variants (both tools run): `.ic v(2)=3` is honoured by RustSpice and ngspice (v(2) starts at 3 V and rises towards 5 V with tau = 1 ms: 3 + 2*(1-exp(-t/1ms))); `abc`, `inf` and an empty value are dropped by RustSpice in the same way. Same behaviour class as rob_hs_syn_param_no_value and rob_hs_syn_model_empty_param_value: an unparsable value is skipped without any diagnostic [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 5
R1 1 2 1k
C1 2 0 1u
.ic v(2)=nan
.tran 20u 500u
.end
