C and L value spellings with equal time constants (tau = 1 ms)
* bench.id: cap_el_val_suffix_cl_tran
* bench.group: capability
* bench.title: C and L value spellings with equal time constants (tau = 1 ms)
* bench.probes: v(c1), v(c2), v(c3), v(c4), v(c5), v(c6), v(l1n), v(l2n), v(l3n)
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: parser, passive, tran, value_syntax
* bench.note: RC and RL branches all have tau = 1 ms; a mis-parsed suffix changes the time constant of exactly one probe. nF/mH/m forms exercise unit letters after the suffix.
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
R1 in c1 1k
C1 c1 0 1u
R2 in c2 1k
C2 c2 0 1000n
R3 in c3 1k
C3 c3 0 1000000p
R4 in c4 1k
C4 c4 0 0.001m
R5 in c5 1k
C5 c5 0 1e-6
R6 in c6 1k
C6 c6 0 1000nF
R7 in l1n 1k
L1 l1n 0 1000m
R8 in l2n 1k
L2 l2n 0 1000000u
R9 in l3n 1k
L3 l3n 0 1e3mH
.tran 20u 5m
.end
