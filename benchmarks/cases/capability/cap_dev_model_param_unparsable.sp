Diode .model with an unparsable value IS=oops (ngspice rejects it)
* bench.id: cap_dev_model_param_unparsable
* bench.group: capability
* bench.title: Diode .model with an unparsable value IS=oops (ngspice rejects it)
* bench.probes: i(v1)
* bench.expect: loud_error
* bench.tags: model_card, diode, dc, ngspice_error_ok
* bench.note: No expression is involved: RustSpice drops any unparsable model value (verified: N=oops, IS= empty and BV=oops are accepted with exit 0 as well) while ngspice-47 rejects each of them; the same deck with IS=1e-14 N=1.2 runs in both tools. Section 13.1 files silent acceptance of malformed input that ngspice rejects under lenient-malformed-input (siblings rob_hs_ic_nan_value, rob_hs_syn_model_empty_param_value); expect stays loud_error because the value is invalid and ngspice refuses it too. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 a 0 0
D1 a 0 DM
.model DM D(IS=oops N=1.2)
.dc V1 0 0.6 0.05
.end
