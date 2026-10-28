Resistor island (R2, R3 between nodes 3 and 4) with no path to any source or ground
* bench.id: rob_cv_sing_floating_island_op
* bench.group: robustness
* bench.title: Resistor island (R2, R3 between nodes 3 and 4) with no path to any source or ground
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: ill_posed, floating, ngspice_error_ok, caught_panic_noise, op
* bench.note: ngspice-47 prints "Warning: singular matrix: check node 4", tries dynamic and true gmin stepping, then returns the island at 0 V (exit 0); RustSpice refuses with "Singular Jacobian matrix encountered at column/row 0" (names no node/element) after printing a caught faer panic message (caught_panic_noise, cosmetic). The problem is consistent but non-unique (any island potential solves KCL), so a regularised answer WITH a warning would be as acceptable as the refusal; the case pins today's refusal - flip it to either if RustSpice adopts ngspice's regularise-and-warn policy.
V1 1 0 5
R1 1 2 1k
R2 3 4 1k
R3 4 3 2k
.op
.end
