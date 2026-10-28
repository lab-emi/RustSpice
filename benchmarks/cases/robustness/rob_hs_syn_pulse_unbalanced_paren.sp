PULSE( without closing parenthesis, transient
* bench.id: rob_hs_syn_pulse_unbalanced_paren
* bench.group: robustness
* bench.title: PULSE( without closing parenthesis, transient
* bench.probes: v(1), v(2)
* bench.expect: match
* bench.known_gap: silent-ignore-source-waveform
* bench.gap.note: an unclosed PULSE( is silently replaced by a constant 0 V source (v(1) stays 0 V); ngspice-47 parses it like the closed PULSE(0 5 1n 1n 1n 5n 10n) and drives 5 V pulses. With the ')' present RustSpice builds the PULSE
* bench.tags: hostile, syntax, parser, tran, source, silent_risk
* bench.est_rss_mib: 64
* bench.note: ngspice accepts the missing ')'; a loud refusal would also be defensible but would be classified fail_robustness; the silent DC fallback is the defect
V1 1 0 PULSE(0 5 1n 1n 1n 5n 10n
R1 1 2 1k
R2 2 0 1k
.tran 1n 20n
.end
