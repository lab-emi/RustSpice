R9 2 0 1k
* bench.id: rob_hs_ff_first_line_is_element
* bench.group: robustness
* bench.title: first line is a resistor card (it is the title and must not be parsed)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: file, parser, silent_risk
* bench.est_rss_mib: 64
* bench.note: line 1 is always the title, so the load R9 (1k, node 2 to ground) is NOT part of the circuit: v(1)=5 V, v(2)=2.5 V, i(v1)=-2.5 mA in both tools. A tool that parsed line 1 would see R2 in parallel with R9 and return v(2)=1.667 V. Redesigned in calibration: the earlier V1 title left every probe at 0 V, so the comparison was vacuous
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
