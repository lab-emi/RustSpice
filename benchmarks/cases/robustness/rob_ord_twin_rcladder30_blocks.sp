Two identical 30-stage RC ladders on one source, block B cards shuffled (seed 3)
* bench.id: rob_ord_twin_rcladder30_blocks
* bench.group: robustness
* bench.title: Two identical 30-stage RC ladders on one source, block B cards shuffled (seed 3)
* bench.probes: v(a1), v(a3), v(a5), v(a10), v(a15), v(a20), v(b1), v(b3), v(b5), v(b10), v(b15), v(b20)
* bench.tol.norm: 2e-4
* bench.floor: 5e-4
* bench.expect: match
* bench.oracle: tran_lti
* bench.size: 63
* bench.est_rss_mib: 20
* bench.tags: linear, tran, ladder, rc, card_order, twin_blocks, silent_risk
* bench.note: block A (a1..a30) is in natural card order, block B (b1..b30) is the same circuit with its cards shuffled by random.Random(3); physically v(aK) == v(bK) for every K, so checks/numbering_order_invariance/check_twin_blocks.py needs no external reference; here both blocks are also compared with ngspice and the exact oracle
V1 in 0 PULSE(0 1 1n 100p 100p 12n 50n)
RA1 in a1 1k
CA1 a1 0 1p
RA2 a1 a2 1k
CA2 a2 0 1p
RA3 a2 a3 1k
CA3 a3 0 1p
RA4 a3 a4 1k
CA4 a4 0 1p
RA5 a4 a5 1k
CA5 a5 0 1p
RA6 a5 a6 1k
CA6 a6 0 1p
RA7 a6 a7 1k
CA7 a7 0 1p
RA8 a7 a8 1k
CA8 a8 0 1p
RA9 a8 a9 1k
CA9 a9 0 1p
RA10 a9 a10 1k
CA10 a10 0 1p
RA11 a10 a11 1k
CA11 a11 0 1p
RA12 a11 a12 1k
CA12 a12 0 1p
RA13 a12 a13 1k
CA13 a13 0 1p
RA14 a13 a14 1k
CA14 a14 0 1p
RA15 a14 a15 1k
CA15 a15 0 1p
RA16 a15 a16 1k
CA16 a16 0 1p
RA17 a16 a17 1k
CA17 a17 0 1p
RA18 a17 a18 1k
CA18 a18 0 1p
RA19 a18 a19 1k
CA19 a19 0 1p
RA20 a19 a20 1k
CA20 a20 0 1p
RA21 a20 a21 1k
CA21 a21 0 1p
RA22 a21 a22 1k
CA22 a22 0 1p
RA23 a22 a23 1k
CA23 a23 0 1p
RA24 a23 a24 1k
CA24 a24 0 1p
RA25 a24 a25 1k
CA25 a25 0 1p
RA26 a25 a26 1k
CA26 a26 0 1p
RA27 a26 a27 1k
CA27 a27 0 1p
RA28 a27 a28 1k
CA28 a28 0 1p
RA29 a28 a29 1k
CA29 a29 0 1p
RA30 a29 a30 1k
CA30 a30 0 1p
CB11 b11 0 1p
CB9 b9 0 1p
CB20 b20 0 1p
RB15 b14 b15 1k
CB28 b28 0 1p
RB12 b11 b12 1k
RB6 b5 b6 1k
RB14 b13 b14 1k
RB4 b3 b4 1k
RB24 b23 b24 1k
CB2 b2 0 1p
CB4 b4 0 1p
RB2 b1 b2 1k
CB25 b25 0 1p
CB14 b14 0 1p
CB6 b6 0 1p
CB16 b16 0 1p
RB19 b18 b19 1k
RB11 b10 b11 1k
CB7 b7 0 1p
CB22 b22 0 1p
CB10 b10 0 1p
CB30 b30 0 1p
RB29 b28 b29 1k
RB17 b16 b17 1k
CB15 b15 0 1p
RB27 b26 b27 1k
CB1 b1 0 1p
RB10 b9 b10 1k
CB21 b21 0 1p
CB3 b3 0 1p
RB26 b25 b26 1k
RB25 b24 b25 1k
RB13 b12 b13 1k
CB17 b17 0 1p
CB26 b26 0 1p
RB23 b22 b23 1k
CB5 b5 0 1p
CB13 b13 0 1p
RB22 b21 b22 1k
CB23 b23 0 1p
CB29 b29 0 1p
CB24 b24 0 1p
RB7 b6 b7 1k
RB8 b7 b8 1k
CB18 b18 0 1p
RB9 b8 b9 1k
CB27 b27 0 1p
RB1 in b1 1k
RB28 b27 b28 1k
RB3 b2 b3 1k
RB30 b29 b30 1k
RB21 b20 b21 1k
RB16 b15 b16 1k
RB20 b19 b20 1k
CB12 b12 0 1p
RB5 b4 b5 1k
RB18 b17 b18 1k
CB19 b19 0 1p
CB8 b8 0 1p
.tran 10p 20n
.end
