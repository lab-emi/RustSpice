Operating point of PWL and EXP sources (value at t = 0: 1.5 V and 1 V)
* bench.id: acc_dc_op_pwl_exp_sources
* bench.group: accuracy
* bench.title: Operating point of PWL and EXP sources (value at t = 0: 1.5 V and 1 V)
* bench.probes: v(a), v(b)
* bench.expect: match
* bench.tags: linear, op, source_waveform, silent_risk
* bench.note: ngspice uses the t = 0 value of the waveform for .op. Tight ngspice gives exactly v(a) = 1.5 V (first PWL value) and v(b) = 1 V (EXP initial value V1); RustSpice gives 0 V for both with exit 0 and empty stderr. The card `V1 a 0 PWL(0 1.5 1m 4)` alone is also 0 V in RustSpice and adding a `DC 1.5` token makes it read 1.5 V, so only a DC token rescues the source: PWL and EXP themselves are never parsed [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 a 0 PWL(0 1.5 1m 4)
R1 a 0 1k
V2 b 0 EXP(1 3 1u 1u 2u 1u)
R2 b 0 1k
.op
.end
