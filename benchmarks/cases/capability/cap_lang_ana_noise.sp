Noise analysis request
* bench.id: cap_lang_ana_noise
* bench.group: capability
* bench.title: .noise v(out) V1 dec 10 1k 10meg must be refused
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, noise, unsupported_analysis
* bench.note: ngspice-47 computes the noise spectra and the integrated noise (not compared by the suite), so the checkable contract is the refusal; RustSpice prints nothing on stderr (checked) [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 DC 1 AC 1
R1 in out 1k
C1 out 0 1n
.noise v(out) V1 dec 10 1k 10meg
.end
