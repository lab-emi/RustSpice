tran 10n 100n 0 -1n (negative tmax)
* bench.id: rob_hs_tran_tmax_negative
* bench.group: robustness
* bench.title: .tran 10n 100n 0 -1n (negative tmax)
* bench.probes: v(2)
* bench.expect: either
* bench.tags: hostile, tran, args, silent_risk
* bench.est_rss_mib: 64
* bench.note: ambiguous, neither tool can be judged: ngspice-47 does not validate tmax, it aborts by numerical accident ('Timestep too small; time = -1e-10, timestep = -2e-09', one row) while tmax = 0 runs normally on this deck (64 rows), so its refusal is no reference for 'a negative tmax must be refused'; RustSpice ignores the field (raw output bit-identical to the deck without tmax, 11 rows, within 2.2e-3 V of the closed-form RC ramp response, swing 0.47 V) and no spec mandates refusing tmax < 0. If validation of the tmax field is wanted by policy it is lenient-malformed-input, not silent-ignore-ic
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1n
.tran 10n 100n 0 -1n
.end
