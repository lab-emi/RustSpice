classic Mac CR-only line endings for the circuit part
* bench.id: rob_hs_ff_cr_only_line_endings
* bench.group: robustness
* bench.title: classic Mac CR-only line endings for the circuit part
* bench.probes: v(1), v(2), i(v1)
* bench.expect: either
* bench.tags: file, encoding
* bench.est_rss_mib: 64
* bench.note: AMBIGUOUS, no verdict (SUITE_SPEC 14.2 rule 2e). The title and metadata lines are LF terminated, the circuit cards are CR terminated (classic Mac). ngspice-47 evidently does not treat a bare CR as a line end and refuses the deck ('unknown parameter (1)', rc 1); RustSpice treats a bare CR as a line break and returns the evident 5 V divider (v(2)=2.5 V, exit 0). No standard defines bare-CR line ends: ngspice refuses loudly, RustSpice reads the author's evident intent; neither is provably right
V1 1 0 5R1 1 2 1kR2 2 0 1k.op.end