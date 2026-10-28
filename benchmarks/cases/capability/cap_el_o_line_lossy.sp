Lossy transmission line O with an LTRA model
* bench.id: cap_el_o_line_lossy
* bench.group: capability
* bench.title: Lossy transmission line O with an LTRA model
* bench.probes: v(out)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the O card is refused at parse time (`Unsupported device card O1`, exit 1, no panic) where ngspice solves the lossy line; the LTRA .model card is accepted silently
* bench.tags: tline, tran, unsupported
* bench.note: Matched lossy line (Z0 = 50, td = 5 ns, R*len/(2*Z0) = 0.01): the output is the input delayed by 5 ns; the first arrival is 0.5*exp(-0.01) = 0.49502 V and the level settles to the DC divider 50/(50+50+1) = 0.495050 V (ngspice-47: 0.4950495 from 15 ns on). Independent check: the exact frequency-domain solution of the RLGC line (ABCD matrix, source 50 ohm, load 50 ohm) inverted by FFT (dt 50 ps with a 6.5 us window; the same with dt 10 ps) agrees with ngspice-47 (reltol 1e-6, tmax 50 ps) to 2.1e-7 V max, 4e-7 of the 0.5 V swing, far inside the default tol.norm 5e-3. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 PULSE(0 1 0 1n 1n 100n 200n)
R1 in a 50
O1 a 0 out 0 lmod
.model lmod LTRA(r=1 l=250n c=100p len=1)
R2 out 0 50
.tran 1n 100n
.end
