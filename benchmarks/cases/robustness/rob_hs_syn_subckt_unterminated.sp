subckt without .ends before .end
* bench.id: rob_hs_syn_subckt_unterminated
* bench.group: robustness
* bench.title: .subckt without .ends before .end
* bench.probes: v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser, subckt, silent_risk
* bench.est_rss_mib: 64
* bench.note: probe and circuit changed after verification: the old deck kept R2 inside the subckt body, so its v(1) probe was pinned by V1 (vacuous) and R2 would not have been in the top-level circuit even with a closing .ends. Now R2 follows the open .subckt and is silently dropped, R1 only reaches the floating node 2: v(2)=5.0 V, i(v1)=0 in RustSpice. The closed deck (`.ends` after R3) gives v(2)=2.5 V, i(v1)=-2.5e-3 A in RustSpice and ngspice alike (5*1k/2k). An X instance of the unclosed subckt is refused by RustSpice ('Unknown subcircuit', exit 1), loud only by accident [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 5
R1 1 2 1k
.subckt foo a b
R3 a b 1k
R2 2 0 1k
.op
.end
