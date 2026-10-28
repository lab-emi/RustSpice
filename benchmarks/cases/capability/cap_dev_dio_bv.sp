Diode reverse breakdown BV=5 V (default IBV), 1 kohm series
* bench.id: cap_dev_dio_bv
* bench.group: capability
* bench.title: Diode reverse breakdown BV=5 V (default IBV), 1 kohm series
* bench.probes: v(a), i(v1)
* bench.tol.rel: 1e-4
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: BV ignored: v(a) follows the source to -10 V (ngspice clamps at -5.04 V)
* bench.tags: diode, dc, breakdown, model_param
* bench.note: rel 1e-4 (0.5 mV at 5 V): steep breakdown exponential; ngspice default tolerances deviate from the tight run by 2.3e-4 V; evidence: RustSpice output is bit-identical with and without BV (ignored); tight ngspice moves by 4.96 V; an own SPICE3-style BV/IBV breakdown solve (python) equals tight ngspice to 2e-7 V
V1 in 0 0
R1 in a 1k
D1 a 0 DM
.model DM D(IS=1e-14 BV=5)
.dc V1 -10 1 0.25
.end
