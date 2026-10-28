SIN with a non-numeric amplitude
* bench.id: rob_hs_syn_sin_text_arg
* bench.group: robustness
* bench.title: SIN with a non-numeric amplitude
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: the unparsable SIN argument 'abc' is dropped and the remaining arguments shift position: SIN(0 abc 1k) leaves 2 arguments and the source silently falls back to DC 0 V, SIN(0 abc 5 1) is read as a 5 V 1 Hz sine (v(1) reaches 2.94 V at 100 ms); ngspice-47 rejects the card (exit 1, 'circuit not parsed')
* bench.tags: hostile, syntax, parser, source, silent_risk
* bench.est_rss_mib: 64
* bench.note: Secondary mechanism: dropped arguments shift the positions of the others (a PULSE(0 5 abc 1n 1n 5n 10n) is read as a different pulse with 5 V peaks, ngspice exit 1). Implementing a SIN default for fewer than three arguments (cap_el_v_sin_min_args) would turn this deck into a silently wrong sine, not a closed gap; only refusing the unparsable argument closes it
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
V9 3 0 SIN(0 abc 1k)
R9 3 0 1k
.op
.end
