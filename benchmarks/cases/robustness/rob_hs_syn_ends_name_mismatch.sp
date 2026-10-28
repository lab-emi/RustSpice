ends with a different name than its .subckt
* bench.id: rob_hs_syn_ends_name_mismatch
* bench.group: robustness
* bench.title: .ends with a different name than its .subckt
* bench.probes: v(1), i(v1)
* bench.expect: match
* bench.tags: syntax, parser, subckt, lenient_ok
* bench.est_rss_mib: 64
* bench.note: ngspice-47 ignores the name after .ends (i(v1) = -5 mA); RustSpice behaves the same
.subckt foo a b
R1 a b 1k
.ends bar
V1 1 0 5
X1 1 0 foo
.op
.end
