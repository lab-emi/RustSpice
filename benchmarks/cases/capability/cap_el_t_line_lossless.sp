Lossless transmission line T (Z0=50, TD=10n)
* bench.id: cap_el_t_line_lossless
* bench.group: capability
* bench.title: Lossless transmission line T (Z0=50, TD=10n)
* bench.probes: v(out)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the T card is refused at parse time (`Unsupported device card T1`, exit 1, no panic) where ngspice solves the line
* bench.tags: tline, tran, unsupported
* bench.note: Matched line: the output is the input pulse delayed by 10 ns at half amplitude (source 50 ohm, load 50 ohm). Closed form 0.5*pulse(t-10n) agrees with ngspice-47 (reltol 1e-6, tmax 0.05n) to 1e-16. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 PULSE(0 1 0 1n 1n 100n 200n)
R1 in a 50
T1 a 0 out 0 Z0=50 TD=10n
R2 out 0 50
.tran 1n 100n
.end
