Series-stacked voltage sources with mixed polarity (+5, +5, -12, +3)
* bench.id: cap_el_v_stacked_mixed_polarity
* bench.group: capability
* bench.title: Series-stacked voltage sources with mixed polarity (+5, +5, -12, +3)
* bench.probes: v(a), v(b), v(c), v(d), i(v1), i(v2), i(v3), i(v4)
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: source, op, stacked, clamp_rail_linear_anchor
* bench.note: Nodes sit at 5, 10, -2 and 1 V, beyond the range of any single source. The linear solve must not clamp (see the nonlinear clamp-rail gap in the robustness group). The branch currents i(v2), i(v3), i(v4) were added after a mutation check showed R2 (which sits across V3 and carries 6 mA) was invisible to the node-voltage probes: i(v1) = i(v2) = -1 mA, i(v3) = +5 mA (6 mA through R2 minus the 1 mA of R1), i(v4) = -1 mA; a 5 % change of R2 now moves i(v3) by 0.3 mA.
V1 a 0 5
V2 b a 5
V3 c b -12
V4 d c 3
R1 d 0 1k
R2 b c 2k
.op
.end
