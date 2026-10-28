Pole-zero analysis request
* bench.id: cap_lang_ana_pz
* bench.group: capability
* bench.title: .pz in 0 out 0 vol pz on a current-driven RC low-pass must be refused
* bench.probes: v(out)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-analysis
* bench.gap.note: .pz is replaced by a plain DC .op (exit 0, v(out) printed)
* bench.tags: linear, pz, unsupported_analysis
* bench.note: ngspice-47 computes the pole-zero plot (not compared by the suite), so the checkable contract is the refusal; RustSpice prints nothing on stderr (checked)
I1 0 in DC 1m
R0 in 0 10k
R1 in out 1k
C1 out 0 1n
.pz in 0 out 0 vol pz
.end
