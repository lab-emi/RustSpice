Diode model TNOM=50 (simulated at 27 C)
* bench.id: cap_dev_dio_tnom_model
* bench.group: capability
* bench.title: Diode model TNOM=50 (simulated at 27 C)
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, temperature, model_param
* bench.note: slug (soft call): the missing feature is the IS(T) scaling, not an individual parameter: ngspice gives the identical result for .options tnom=50 instead of the model TNOM, RustSpice is bit-identical for both and without TNOM, and .options tnom is already covered by this slug; deleting TNOM=50 makes RustSpice equal tight ngspice to 2e-12 relative; textbook IS rescaling from 50 C to 27 C equals tight ngspice to 1e-5 relative [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 a 0 0
D1 a 0 DM

.model DM D(IS=1e-14 TNOM=50)
.dc V1 0 0.7 0.05
.end
