RC with a small TMAX requested on the .tran card
* bench.id: acc_tr_tmax_request
* bench.group: accuracy
* bench.title: RC (tau = 100 ns) driven by a 1 us pulse with 1 ns edges; .tran 0.5u 4u 0 10n asks for internal steps <= 10 ns
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.known_gap: fixed-step-accuracy
* bench.gap.note: RustSpice steps at the 0.5 us print step (9 rows) over the 1 ns edges and its error is 0.248 of swing; .tran TMAX is ignored too (output bit-identical for tmax 10n, 1n, absent, 2u) but is not what decides the error
* bench.ref.tmax: 2n
* bench.tags: linear, tran, tmax, timing_edge, silent_risk
* bench.est_rss_mib: 16
* bench.note: slug fixed-step-accuracy, not silent-ignore-ic: ngspice is accurate on the 0.5 us grid even WITHOUT tmax (as-is run, no tmax, 114 rows, linearly resampled: 1.4e-3 of swing vs a Radau closed-form truth with breakpoints, inside tol.norm 5e-3; with tmax = 10n: 2.8e-5), so tmax is not what makes ngspice right, while RustSpice's error follows its fixed step (= print step, no breakpoints): vs the same truth 0.248 at tstep 0.5u, 0.19 at 50n, 0.043 at 10n (tmax unchanged); an adaptive-step RustSpice would pass here with tmax still ignored
V1 in 0 PULSE(0 1 0 1n 1n 1u 2u)
R1 in out 1k
C1 out 0 100p
.tran 0.5u 4u 0 10n
.end
