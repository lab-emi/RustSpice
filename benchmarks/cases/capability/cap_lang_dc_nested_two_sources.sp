Nested two-source DC sweep
* bench.id: cap_lang_dc_nested_two_sources
* bench.group: capability
* bench.title: .dc V1 0 2 1 V2 0 1 0.5 sweeps V1 (inner) for each V2 value, 9 points
* bench.probes: v(out)
* bench.known_gap: silent-misparse
* bench.gap.note: nested .dc: the .dc parser reads only the first sweep and drops the rest of the card without a message: second sweep (V2) ignored, 3 points instead of 9
* bench.tags: linear, dc, nested_sweep
* bench.note: Independent truth: v(out) = (V1 + V2)/3 over the 9 points (ngspice matches it); three separate RustSpice sweeps of V1 at V2 = 0, 0.5 and 1 equal the three ngspice blocks exactly, so only the dropped second sweep fails
V1 a 0 0
V2 b 0 0
R1 a out 1k
R2 b out 1k
R3 out 0 1k
.dc V1 0 2 1 V2 0 1 0.5
.end
