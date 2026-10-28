Numeric solver options are accepted
* bench.id: cap_lang_opt_accepted_numeric
* bench.group: capability
* bench.title: a long .options card (reltol abstol vntol itl1 itl2 itl4 pivtol trtol) must parse on a linear divider
* bench.probes: v(out), i(v1)
* bench.tags: linear, op, options, syntax
.options reltol=1e-4 abstol=1e-10 vntol=1e-5 itl1=200 itl2=100 itl4=20 pivtol=1e-13 trtol=7
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.op
.end
