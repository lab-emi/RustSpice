Zener shunt regulator: 10 V, 1 kohm, BV=5.1 V, 2 kohm load
* bench.id: cap_dev_dio_zener_regulator
* bench.group: capability
* bench.title: Zener shunt regulator: 10 V, 1 kohm, BV=5.1 V, 2 kohm load
* bench.probes: v(out), i(v1)
* bench.tol.rel: 1e-5
* bench.expect: match
* bench.tags: diode, op, breakdown, textbook
* bench.note: Gap closed 2026-10-28: RustSpice now models diode reverse breakdown BV/IBV; the evidence below records the reference and the behaviour before the fix. rel 1e-5: RustSpice stops Newton at reltol 1e-6 and the breakdown knee is a steep exponential; evidence: RustSpice output is bit-identical with and without BV/IBV (6.667 V plain divider); tight ngspice moves by 1.59 V; an own SPICE3-style BV/IBV solve (python) gives 5.080787 V = tight ngspice
V1 in 0 10
R1 in out 1k
D1 0 out DZ
RL out 0 2k
.model DZ D(IS=1e-14 BV=5.1 IBV=5m)
.op
.end
