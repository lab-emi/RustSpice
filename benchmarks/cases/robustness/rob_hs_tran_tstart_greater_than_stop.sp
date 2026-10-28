tran 10n 1u 2u (tstart > tstop)
* bench.id: rob_hs_tran_tstart_greater_than_stop
* bench.group: robustness
* bench.title: .tran 10n 1u 2u (tstart > tstop)
* bench.probes: v(2)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: tstart > tstop is accepted: RustSpice simulates and prints the whole 0..1 us transient (101 rows, exit 0, no diagnostic); ngspice-47 rejects it with 'TSTART is invalid, must be less than TSTOP.' (exit 1)
* bench.tags: hostile, tran, args, silent_risk
* bench.est_rss_mib: 64
* bench.note: slug lenient-malformed-input (SUITE_SPEC 13.1): the failing contract is input validation; RustSpice ignores tstart for valid values too (a valid tstart is a separate silent-ignore-ic matter), so honouring tstart would not by itself produce the refusal
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1n
.tran 10n 1u 2u
.end
