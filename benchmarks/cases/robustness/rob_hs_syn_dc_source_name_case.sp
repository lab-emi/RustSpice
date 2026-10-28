dc sweep source named in another case, dot card continued
* bench.id: rob_hs_syn_dc_source_name_case
* bench.group: robustness
* bench.title: .DC names the swept source V1 although the element is written v1, and the card is continued
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: syntax, parser, dc, continuation, lenient_ok
* bench.est_rss_mib: 64
* bench.note: element and sweep card differ only in case and the sweep arguments sit on a '+' line; the sweep of v1 over 0..5 V in 1 V steps must give v(2) = v(1)/2 at all 6 points (non-vacuous: v(2) reaches 2.5 V, i(v1) reaches -2.5 mA)
v1 1 0 0
R1 1 2 1k
R2 2 0 1k
.DC V1
+ 0 5 1
.end
