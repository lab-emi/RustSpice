Diode breakdown knee current IBV=1 uA with BV=5 V
* bench.id: cap_dev_dio_ibv
* bench.group: capability
* bench.title: Diode breakdown knee current IBV=1 uA with BV=5 V
* bench.probes: v(a), i(v1)
* bench.tol.rel: 1e-4
* bench.expect: match
* bench.tags: diode, dc, breakdown, model_param
* bench.note: Gap closed 2026-10-28: RustSpice now models diode reverse breakdown BV/IBV; the evidence below records the reference and the behaviour before the fix. rel 1e-4 (0.5 mV at 5 V): steep breakdown exponential; ngspice default tolerances deviate from the tight run by 2.3e-4 V; evidence: RustSpice output is bit-identical with and without BV/IBV (ignored); tight ngspice moves by 4.78 V (IBV=1u shifts the clamp from 5.04 V to 5.22 V); an own SPICE3-style BV/IBV solve (python) equals tight ngspice to 2e-7 V
V1 in 0 0
R1 in a 1k
D1 a 0 DM
.model DM D(IS=1e-14 BV=5 IBV=1e-6)
.dc V1 -10 1 0.25
.end
