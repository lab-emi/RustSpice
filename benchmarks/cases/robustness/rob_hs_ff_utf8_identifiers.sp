non-ASCII element and node names (UTF-8)
* bench.id: rob_hs_ff_utf8_identifiers
* bench.group: robustness
* bench.title: non-ASCII element and node names (UTF-8)
* bench.probes: v(1), v(2)
* bench.expect: match
* bench.tags: file, encoding, parser
* bench.est_rss_mib: 64
* bench.note: ngspice mangles the vector names of non-ASCII nodes, so only the ASCII-named nodes v(1), v(2) are compared; v(2)=3.333 V, v(1)=5 V
Vé 1 0 5
RΩ 1 2 1k
R2 2 µout 1k
R3 µout 0 1k
.op
.end
