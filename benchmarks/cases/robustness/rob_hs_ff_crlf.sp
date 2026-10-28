CRLF line endings
* bench.id: rob_hs_ff_crlf
* bench.group: robustness
* bench.title: CRLF line endings
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: file, encoding, lenient_ok
* bench.est_rss_mib: 64
* bench.note: title and circuit lines are CRLF terminated; the bench.* metadata lines stay LF so that a plain LF metadata parser works
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
