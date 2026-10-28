DC sweep of a resistor value (.dc R2 1k 3k 1k): ngspice sweeps the element value
* bench.id: acc_dc_grid_sweep_resistor_value
* bench.group: accuracy
* bench.title: DC sweep of a resistor value (.dc R2 1k 3k 1k): ngspice sweeps the element value
* bench.probes: v(out)
* bench.expect: match
* bench.tags: linear, dc, sweep, capability, unsupported_feature
* bench.note: closed form v(out) = 3 V * R2 / (1k + R2) = 1.5, 2.0, 2.25 V, which ngspice reproduces; per policy 13.1 a loud refusal of a feature ngspice computes is a known gap (unsupported-syntax-loud), not an accepted outcome: when implemented the case must flip to gap_closed
V1 in 0 3
R1 in out 1k
R2 out 0 1k
.dc R2 1k 3k 1k
.end
