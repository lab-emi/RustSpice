island with no connection to the ground reference
* bench.id: rob_hs_sing_floating_island
* bench.group: robustness
* bench.title: island with no connection to the ground reference
* bench.probes: v(1)
* bench.expect: either
* bench.tags: caught_panic_noise, hostile, singular, dc, floating
* bench.est_rss_mib: 64
* bench.note: a solution exists but is not unique (common-mode of the island is free): ngspice warns "singular matrix" and returns one of them (v(3)=2.002 V, v(4)=0.002 V, i(v2)=-2 mA all satisfy KCL, the 2 V difference is right); RustSpice refuses with SingularMatrix (the faer panic is caught but prints noise to stderr). Both defensible: ambiguous
V1 1 0 5
R1 1 0 1k
V2 3 4 2
R3 3 4 1k
.op
.end
