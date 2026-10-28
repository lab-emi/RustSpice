Pulse source into an ideal inductor into an ideal battery (DC-singular initial OP)
* bench.id: rob_cv_sing_vl_loop_tran
* bench.group: robustness
* bench.title: Pulse source into an ideal inductor into an ideal battery (DC-singular initial OP)
* bench.probes: v(in)
* bench.expect: loud_error
* bench.tags: ill_posed, vl_loop, ngspice_error_ok, caught_panic_noise, tran
* bench.note: ngspice-47 warns "singular matrix: check node l1#branch" during the OP (dynamic gmin, true gmin and source stepping are all attempted) and then integrates the ramp; RustSpice refuses at the OP ("Singular Jacobian matrix encountered at column/row 0") after printing a caught faer panic message (caught_panic_noise, cosmetic). The initial DC point has no solution (a 0 V source and a 2.5 V battery joined by a DC short), so refusing is correct. The same circuit with 0.01 ohm in series solves in both tools (v(in) agrees to 7e-12).
V1 in 0 PULSE(0 5 0 1n 1n 5u 10u)
L1 in out 10u
Vbat out 0 2.5
.tran 20n 20u
.end
