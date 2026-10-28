Node touched only by a 1 mA current source (KCL has no solution)
* bench.id: rob_cv_sing_isrc_cutset_op
* bench.group: robustness
* bench.title: Node touched only by a 1 mA current source (KCL has no solution)
* bench.probes: v(1)
* bench.expect: either
* bench.tags: ill_posed, current_cutset, silent_risk, op
* bench.note: No DC solution exists (the MNA matrix has rank 2, [A|b] rank 3), so every returned vector violates KCL at node 1 by the full 1 mA and no value can be called correct. Verified with both tools: ngspice-47 exits 0 and returns v(1)=1 mV with NO warning (as-is and tight options), RustSpice exits 0 and returns v(1)=1e9 V (=I/gmin), also without a diagnostic. Neither tool refuses or warns, so neither can be judged (policy 13.1: input ngspice does not reject is not expect: loud_error), hence expect: either. The legitimate improvement, a diagnostic for a node without a DC path, cannot be expressed by the current expectations. The well-posed v(2)=1 V and i(v1)=-1 mA agree in both. Contrast rob_cv_sing_cap_only_node_op (consistent but non-unique).
I1 0 1 1m
V1 2 0 1
R1 2 0 1k
.op
.end
