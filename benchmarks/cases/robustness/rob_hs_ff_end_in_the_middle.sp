end followed by more cards and a second .end
* bench.id: rob_hs_ff_end_in_the_middle
* bench.group: robustness
* bench.title: .end followed by more cards and a second .end
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: file, parser, lenient_ok
* bench.est_rss_mib: 64
* bench.note: ngspice-47 ignores a first .end that is followed by more cards (v(2)=2.5 V); RustSpice behaves the same
V1 1 0 5
R1 1 2 1k
.end
R2 2 0 1k
.op
.end
