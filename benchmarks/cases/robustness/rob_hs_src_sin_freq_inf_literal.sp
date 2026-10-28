SIN with the literal inf as frequency
* bench.id: rob_hs_src_sin_freq_inf_literal
* bench.group: robustness
* bench.title: SIN with the literal inf as frequency
* bench.probes: v(2)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: the unparsable frequency 'inf' is dropped and V1 degrades to a 0 V source: 201 rows, v(2) = 0 V at every time point (all finite) with exit 0; 'abc' behaves identically (verified); ngspice-47 exits 1 ('unknown parameter (inf)')
* bench.tags: hostile, source, nonfinite, tran, silent_risk
* bench.est_rss_mib: 64
* bench.note: Sibling of rob_hs_src_sin_freq_inf (frequency 1e999, which parses to +inf and yields NaN rows) and of rob_hs_val_v_nan / rob_hs_val_i_inf (value tokens dropped as unparsable). Here the token inf is not a number for either tool: ngspice-47 rejects the card, RustSpice drops the last SIN argument and treats the source as DC 0 V. Not a vacuous probe: with a valid frequency (SIN(0 1 1e6)) v(2) swings to 0.86 V in the same RC circuit (verified), so the all-zero output here is a changed circuit. expect stays loud_error because the value is invalid and ngspice refuses it too (section 13.1).
V1 1 0 SIN(0 1 inf)
R1 1 2 1k
C1 2 0 100p
.tran 10n 2u
.end
