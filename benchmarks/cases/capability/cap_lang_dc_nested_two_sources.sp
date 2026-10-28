Nested two-source DC sweep
* bench.id: cap_lang_dc_nested_two_sources
* bench.group: capability
* bench.title: .dc V1 0 2 1 V2 0 1 0.5 sweeps V1 (inner) for each V2 value, 9 points
* bench.probes: v(out)
* bench.tags: linear, dc, nested_sweep
* bench.note: Independent truth: v(out) = (V1 + V2)/3 over the 9 points (ngspice matches it); three separate RustSpice sweeps of V1 at V2 = 0, 0.5 and 1 equal the three ngspice blocks exactly, so only the dropped second sweep fails [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 a 0 0
V2 b 0 0
R1 a out 1k
R2 b out 1k
R3 out 0 1k
.dc V1 0 2 1 V2 0 1 0.5
.end
