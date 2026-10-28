Current sources with SIN and PULSE waveforms at .op (no DC attribute) take the t=0 value
* bench.id: cap_el_i_op_waveform_t0
* bench.group: capability
* bench.title: Current sources with SIN and PULSE waveforms at .op take the t=0 value
* bench.probes: v(a), v(b), v(c), v(d), v(e)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.floor: 1e-6
* bench.expect: match
* bench.tags: source, op, waveform, current_source, precedence
* bench.note: A source with a waveform and no DC attribute contributes its time-zero value at .op: SIN gives vo, PULSE gives v1 (also while the delay td is still pending), SIN with delay and damping gives vo. Each load is 1k so v = I*1k: v(a)=1, v(b)=2, v(c)=3, v(d)=0.5, v(e)=-1.5 V (the last is a PULSE whose v1 is negative).
I1 0 a SIN(1m 2m 1meg)
R1 a 0 1k
I2 0 b PULSE(2m 5m 0 1n 1n 1u 2u)
R2 b 0 1k
I3 0 c PULSE(3m 5m 1u 1n 1n 1u 3u)
R3 c 0 1k
I4 0 d SIN(0.5m 1m 1meg 2u 100k)
R4 d 0 1k
I5 0 e PULSE(-1.5m 2m 10n 1n 1n 1u 3u)
R5 e 0 1k
.op
.end
