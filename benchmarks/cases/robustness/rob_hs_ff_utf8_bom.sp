UTF-8 BOM before the title
* bench.id: rob_hs_ff_utf8_bom
* bench.group: robustness
* bench.title: UTF-8 byte-order mark before the title line
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: file, encoding, lenient_ok
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
