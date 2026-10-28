include cycle A -> B -> A (targets rob_hs_include_loop_[ab].lib next to the deck)
* bench.id: rob_hs_include_loop
* bench.group: robustness
* bench.title: .include cycle A -> B -> A (targets rob_hs_include_loop_[ab].lib next to the deck)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .include is ignored (exit 0, v(2)=2.5 V), so the cycle goes unnoticed; a simulator that honours .include must stop the cycle with an error, ngspice-47 segfaults on it (rc 139, no reference numbers). Confirmed mechanism: an include of an existing, acyclic file with an element is ignored too
* bench.tags: hostile, include, recursion, resource, silent_risk, ngspice_crash
* bench.est_rss_mib: 64
* bench.note: paths are relative: ngspice resolves them against its working directory; any resolution failure is also an error for ngspice. Weak and derivative case: ngspice-47 itself segfaults on the cycle (rc 139, no reference numbers) and cycle detection cannot be tested until .include is implemented at all (the defect proper is shown by rob_hs_include_missing_file and cap_lang_inc_file); the 2.5 V RustSpice prints is also what include-once semantics would give
.include rob_hs_include_loop_a.lib
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
