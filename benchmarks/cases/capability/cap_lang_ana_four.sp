Fourier analysis request
* bench.id: cap_lang_ana_four
* bench.group: capability
* bench.title: .four 1k v(out) next to a valid .tran (ngspice -b -r drops the card too, informational)
* bench.probes: v(out)
* bench.expect: either
* bench.tags: linear, tran, four, unsupported_analysis
* bench.note: DECK_BUG corrected: ngspice -b -r itself drops .four ('.fourier line ignored since rawfile was produced' on its console, exit 0, waveform written), so a refusal (loud_error) is stricter than the reference; the waveforms agree and the tran numbers are fine, and the harness judges only the exit code and cannot see a warning. What remains is a diagnostics gap that no status can express: RustSpice drops the card without any message (exit 0, empty stderr, raw output bit-identical to the deck without it; a garbage '.four' argument line is dropped the same way). The wanted behaviour is 'warn or refuse'
V1 in 0 SIN(0 1 1k)
R1 in out 1k
C1 out 0 100n
.tran 10u 3m
.four 1k v(out)
.end
