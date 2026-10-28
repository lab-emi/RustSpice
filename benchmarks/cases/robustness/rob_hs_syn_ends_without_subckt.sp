ends with no open .subckt
* bench.id: rob_hs_syn_ends_without_subckt
* bench.group: robustness
* bench.title: .ends with no open .subckt
* bench.probes: v(1), v(2), i(v1)
* bench.expect: either
* bench.tags: hostile, syntax, parser, subckt, lenient_ok
* bench.est_rss_mib: 64
* bench.note: the deck is a valid 5 V divider plus one stray .ends line. Neither side can be judged: RustSpice accepts and ignores the .ends and returns exactly the numbers of the deck without it (v(2)=2.5 V = 5*1k/2k, exit 0), and I found no variant in which that reading misleads (a doubled .ends after a closed subckt gives 2.5 V, a commented-out `*.subckt` header gives the literal reading 1.667 V, the same as the deck with no header at all, and an X instance of an unclosed subckt is still refused loudly); ngspice-47 aborts with 'Mismatch of .subckt ... .ends statements' (exit 1). Only ngspice-47 was run, so whether another SPICE dialect would reject it is unverified. Retagged from known_gap lenient-malformed-input (loud_error) to either: accepted-but-harmless, not a wrong number
V1 1 0 5
R1 1 2 1k
R2 2 0 1k

.ends
.op
.end
