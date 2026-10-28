E (VCVS) card without control nodes and gain
* bench.id: rob_hs_syn_e_too_few_args
* bench.group: robustness
* bench.title: E (VCVS) card without control nodes and gain
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser, ngspice_crash
* bench.est_rss_mib: 64
* bench.note: ngspice-47 prints 'not enough parameters' and then segfaults (exit 139); RustSpice must refuse cleanly. Weak pass today: RustSpice refuses EVERY E card ('Unsupported device card', also a valid E9 3 0 2 0 2 that ngspice solves), so the arity check is not exercised until E is implemented; the expectation (refuse the malformed card) stays correct then
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
E9 3 0 1
R9 3 0 1k
.op
.end
