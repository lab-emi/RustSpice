DC sweep of a source that does not exist
* bench.id: cap_lang_dc_source_missing
* bench.group: capability
* bench.title: .dc VX 0 1 0.5 names a source absent from the circuit
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, dc, invalid_input, ngspice_error_ok
V1 in 0 1
R1 in out 1k
R2 out 0 1k
.dc VX 0 1 0.5
.end
