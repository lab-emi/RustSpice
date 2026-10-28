Continuation line inside PULSE arguments
* bench.id: cap_lang_syn_cont_pulse
* bench.group: capability
* bench.title: PULSE(0 5 1u 10n / + 10n 3u 10u) with the argument list split across lines
* bench.probes: v(out)
* bench.tags: linear, tran, syntax, continuation, waveform
V1 in 0 PULSE(0 5 1u 10n
+ 10n 3u 10u)
R1 in out 1k
C1 out 0 100p
.tran 10n 8u
.end
