cards after .end
* bench.id: rob_hs_ff_text_after_end
* bench.group: robustness
* bench.title: cards after .end
* bench.probes: v(1), v(2), i(v1)
* bench.expect: either
* bench.tags: file, parser, hostile
* bench.est_rss_mib: 64
* bench.note: AMBIGUOUS, no verdict (SUITE_SPEC 14.2 rule 2e). Text after .end has no defined meaning in SPICE. ngspice-47 keeps reading after .end (see rob_hs_ff_end_in_the_middle, where a valid R2 behind the first .end is used by both tools) and refuses the garbage line 'this is not spice @@@' ('transmission line z0 must be given', rc 1); RustSpice also parses on and refuses it as 'Unsupported device card'. Both refuse loudly, but a tool that stopped at .end and returned numbers would be equally defensible, so the refusal is not asserted
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
R9 2 0 1
this is not spice @@@
.tran 1n 1u
