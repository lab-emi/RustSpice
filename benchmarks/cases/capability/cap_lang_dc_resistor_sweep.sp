DC sweep of a resistor value
* bench.id: cap_lang_dc_resistor_sweep
* bench.group: capability
* bench.title: .dc R1 1k 3k 1k sweeps a resistor instead of a source
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud refusal: Topology error: Source `R1` not found in circuit
* bench.tags: linear, dc
* bench.note: ngspice-47 sweeps the resistor: v(out) = 6 V * 1k/(R1+1k) = 3, 2, 1.5 V at R1 = 1k, 2k, 3k (hand-checked)
V1 in 0 6
R1 in out 1k
R2 out 0 1k
.dc R1 1k 3k 1k
.end
