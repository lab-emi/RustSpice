Current sources with PULSE and SIN waveforms
* bench.id: cap_el_i_waveforms_tran
* bench.group: capability
* bench.title: Current sources with PULSE and SIN waveforms
* bench.probes: v(a), v(b)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: source, tran, waveform, current_source
* bench.note: Same waveform code path as voltage sources but for I; v = I*1k.
I1 0 a PULSE(0 1m 1u 100n 100n 1u 3u)
R1 a 0 1k
I2 0 b SIN(0 1m 1meg 0.5u)
R2 b 0 1k
.tran 10n 6u
.end
