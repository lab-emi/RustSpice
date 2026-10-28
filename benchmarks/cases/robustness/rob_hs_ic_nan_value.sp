ic v(2)=nan
* bench.id: rob_hs_ic_nan_value
* bench.group: robustness
* bench.title: .ic v(2)=nan
* bench.probes: v(2)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: the .ic card with the unparsable value nan is dropped silently (v(2) starts at the 5 V operating point and stays there, exit 0); a valid .ic v(2)=3 IS honoured (v(2) starts at 3 V), so only the malformed card is lost; ngspice rejects the netlist
* bench.tags: hostile, ic, tran, nonfinite, silent_risk
* bench.est_rss_mib: 64
* bench.note: the registered silent-ignore-ic slug covers valid ic features that are ignored; here the card is malformed and silently discarded. ngspice-47 stops with 'Undefined parameter [nan]' / 'Cannot compute substitute' (exit 1). Variants (both tools run): `.ic v(2)=3` is honoured by RustSpice and ngspice (v(2) starts at 3 V and rises towards 5 V with tau = 1 ms: 3 + 2*(1-exp(-t/1ms))); `abc`, `inf` and an empty value are dropped by RustSpice in the same way. Same behaviour class as rob_hs_syn_param_no_value and rob_hs_syn_model_empty_param_value: an unparsable value is skipped without any diagnostic
V1 1 0 5
R1 1 2 1k
C1 2 0 1u
.ic v(2)=nan
.tran 20u 500u
.end
