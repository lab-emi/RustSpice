R-2R ladder DAC, 12 bit, code 101101001011 driven at 2.5 V: v(x0) = 2.5 * 2891 / 4096
* bench.id: acc_dc_lin_r2r_ladder_12bit
* bench.group: accuracy
* bench.title: R-2R ladder DAC, 12 bit, code 101101001011 driven at 2.5 V: v(x0) = 2.5 * 2891 / 4096
* bench.probes: v(x0), v(x1), v(x5), v(x11)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.oracle: dc_linear
* bench.size: 24
* bench.tags: linear, op, ladder, dac
* bench.note: tol.rel/tol.abs tightened (f64 LU solves agree with tight ngspice to ~1e-13)
R2_0 x0 sw0 20k
Vb0 sw0 0 2.5
Rr0 x0 x1 10k
R2_1 x1 sw1 20k
Vb1 sw1 0 0
Rr1 x1 x2 10k
R2_2 x2 sw2 20k
Vb2 sw2 0 2.5
Rr2 x2 x3 10k
R2_3 x3 sw3 20k
Vb3 sw3 0 2.5
Rr3 x3 x4 10k
R2_4 x4 sw4 20k
Vb4 sw4 0 0
Rr4 x4 x5 10k
R2_5 x5 sw5 20k
Vb5 sw5 0 2.5
Rr5 x5 x6 10k
R2_6 x6 sw6 20k
Vb6 sw6 0 0
Rr6 x6 x7 10k
R2_7 x7 sw7 20k
Vb7 sw7 0 0
Rr7 x7 x8 10k
R2_8 x8 sw8 20k
Vb8 sw8 0 2.5
Rr8 x8 x9 10k
R2_9 x9 sw9 20k
Vb9 sw9 0 0
Rr9 x9 x10 10k
R2_10 x10 sw10 20k
Vb10 sw10 0 2.5
Rr10 x10 x11 10k
R2_11 x11 sw11 20k
Vb11 sw11 0 2.5
Rterm x11 0 20k
.op
.end
