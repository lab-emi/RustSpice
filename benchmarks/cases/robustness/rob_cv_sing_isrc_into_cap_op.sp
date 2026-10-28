1 uA current source feeding only a capacitor (no DC path), .op
* bench.id: rob_cv_sing_isrc_into_cap_op
* bench.group: robustness
* bench.title: 1 uA current source feeding only a capacitor (no DC path), .op
* bench.probes: v(1)
* bench.expect: either
* bench.tags: ill_posed, current_cutset, silent_risk, ngspice_error_ok, op
* bench.note: With the capacitor open at DC the node equation reads 0*v(1) = 1 uA (rank 0 vs 1): no DC solution exists and any number is wrong by 100 % of the source current. Verified with both tools: ngspice-47 prints "singular matrix: check node 1" warnings, "Dynamic gmin stepping failed" and the true-gmin attempts, then exits 0 with a fallback value v(1)=0.01 V (as-is and tight); RustSpice exits 0 silently with v(1)=1e6 V (=I/gmin). Both tools return arbitrary numbers (ngspice at least warns in its log, which the ngspice_error_ok tag tolerates), so neither can be judged on the value and expect: either. A RustSpice diagnostic or refusal would be the legitimate improvement; the current expectations cannot express it.
I1 0 1 1u
C1 1 0 1n
.op
.end
