DC sweep endpoint expression that starts with a number
* bench.id: cap_lang_dc_expr_numfirst
* bench.group: capability
* bench.title: .dc V1 0 {2*vmax} 0.5 with vmax = 1.5: the expression starts with a digit and the sweep must end at 3 V (7 points)
* bench.probes: v(out)
* bench.known_gap: expr-truncation
* bench.gap.note: {2*vmax} is cut to its first number: the sweep ends at 2 V (5 points) instead of 3 V (7 points), exit 0, no message
* bench.tags: linear, dc, param, expression
* bench.note: vmax was 1 before calibration, where the truncated value 2 equals 2*vmax and the deck could not see the defect; with vmax = 1.5 the ngspice sweep is 0..3 V in 0.5 V steps (7 points, v(out) = V/2, hand-checked)
.param vmax=1.5
V1 in 0 0
R1 in out 1k
R2 out 0 1k
.dc V1 0 {2*vmax} 0.5
.end
