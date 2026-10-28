C card without a value
* bench.id: rob_hs_syn_c_missing_value
* bench.group: robustness
* bench.title: C card without a value
* bench.probes: v(1), v(2), i(v1)
* bench.expect: either
* bench.tags: hostile, syntax, parser, ngspice_warns
* bench.est_rss_mib: 64
* bench.note: AMBIGUOUS, no verdict (SUITE_SPEC 14.2 rule 2e). ngspice-47 logs "Warning: 'c9 2 0' is not a valid capacitor instance line, ignored!", drops the card and solves the rest (rc 0, v(2)=2.5 V); RustSpice refuses with a ParseError. SPICE defines no value-less C card and both reactions are loud, so neither is provably right; a silent misparse (RustSpice inventing a value) would appear as outcome mismatch instead of loud_error
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
C9 2 0
.op
.end
