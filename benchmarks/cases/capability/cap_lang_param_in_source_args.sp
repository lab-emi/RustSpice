Parameters inside waveform arguments
* bench.id: cap_lang_param_in_source_args
* bench.group: capability
* bench.title: PULSE(0 {vhi} {td} {tr} {tr} {pw} {per}) takes every argument from .param
* bench.probes: v(out)
* bench.tags: linear, tran, param, waveform
.param vhi=5 td=1u tr=10n pw=3u per=10u
V1 in 0 PULSE(0 {vhi} {td} {tr} {tr} {pw} {per})
R1 in out 1k
C1 out 0 100p
.tran 10n 8u
.end
