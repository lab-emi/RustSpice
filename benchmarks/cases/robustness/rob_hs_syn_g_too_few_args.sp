G (VCCS) card without control nodes and gain
* bench.id: rob_hs_syn_g_too_few_args
* bench.group: robustness
* bench.title: G (VCCS) card without control nodes and gain
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser, ngspice_crash
* bench.est_rss_mib: 64
* bench.note: ngspice-47 prints 'not enough parameters' and then segfaults (exit 139); RustSpice must refuse cleanly. RustSpice refuses for the intended reason (its own arity check: 'VCCS requires G<name> <n+> <n-> <nc+> <nc-> <gm>'; the valid form G9 2 0 1 0 1m is accepted)
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
G9 2 0 1
.op
.end
