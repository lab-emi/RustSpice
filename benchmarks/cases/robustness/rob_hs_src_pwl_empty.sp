PWL() without points
* bench.id: rob_hs_src_pwl_empty
* bench.group: robustness
* bench.title: PWL() without points
* bench.probes: v(2)
* bench.expect: loud_error
* bench.tags: hostile, source, tran, silent_risk
* bench.est_rss_mib: 64
* bench.note: The check is masked by a second gap: every PWL source, a valid PWL(0 0 1u 1 2u 0) included, is DC 0 V in RustSpice (ngspice peak 1.0 V), so this deck cannot separate empty-list validation from PWL not being parsed. It stays a gap until RustSpice refuses the empty list; parsing PWL alone does not close it [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 PWL()
R1 1 2 1k
C1 2 0 100p
.tran 10n 2u
.end
