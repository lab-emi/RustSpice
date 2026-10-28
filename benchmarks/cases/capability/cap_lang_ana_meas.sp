Measurement request
* bench.id: cap_lang_ana_meas
* bench.group: capability
* bench.title: .meas tran vmax max v(out) next to a valid .tran (ngspice -b -r drops the card too, informational)
* bench.probes: v(out)
* bench.expect: either
* bench.tags: linear, tran, meas, unsupported_analysis
* bench.note: DECK_BUG corrected: ngspice -b -r itself drops .meas ('No .measure possible in batch mode (-b) with -r rawfile set!' on its console, exit 0, waveform written), so a refusal (loud_error) is stricter than the reference; the waveforms agree and the tran numbers are fine, and the harness judges only the exit code and cannot see a warning. What remains is a diagnostics gap that no status can express: RustSpice drops the card without any message (exit 0, empty stderr, raw output bit-identical to the deck without it). The wanted behaviour is 'warn or refuse'
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 10n 5u
.meas tran vmax max v(out)
.end
