Small-signal AC analysis request
* bench.id: cap_lang_ana_ac
* bench.group: capability
* bench.title: .ac dec 10 1k 10meg on an RC low-pass is outside RustSpice's analyses and must be refused
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, ac, unsupported_analysis
* bench.note: ngspice-47 computes the AC sweep (41 complex points, not compared by the suite), so the checkable contract is the refusal: a DC/transient tool must not answer an AC request with a substituted .op; RustSpice prints nothing on stderr (checked) [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 DC 1 AC 1
R1 in out 1k
C1 out 0 1n
.ac dec 10 1k 10meg
.end
