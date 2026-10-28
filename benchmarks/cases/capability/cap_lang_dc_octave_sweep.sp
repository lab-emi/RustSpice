DC sweep with octave spacing
* bench.id: cap_lang_dc_octave_sweep
* bench.group: capability
* bench.title: .dc V1 OCT 1 1 8 (octave sweep form)
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, dc, ngspice_error_ok
* bench.note: ngspice-47's .dc only knows 'src start stop step': it rejects the OCT form too (exit 1, 'circuit not parsed', no rawfile), so both tools refuse and refusing is correct
V1 in 0 1
R1 in out 1k
R2 out 0 1k
.dc V1 OCT 1 1 8
.end
