two voltage sources of different value in parallel
* bench.id: rob_hs_sing_vsource_loop_inconsistent
* bench.group: robustness
* bench.title: two voltage sources of different value in parallel
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: caught_panic_noise, hostile, singular, dc
* bench.est_rss_mib: 64
* bench.note: RustSpice prints three 'panicked at faer ... explicit panic' blocks to stderr (caught) before 'Error: SingularMatrix'
V1 1 0 5
V2 1 0 3
R1 1 0 1k
.op
.end
