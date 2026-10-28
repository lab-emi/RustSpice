Transient superposition on three 30-stage RC ladders: PULSE+SIN versus PULSE alone plus SIN alone
* bench.id: acc_rel_tran_superposition_rcladder30
* bench.group: accuracy
* bench.title: Transient superposition: v(u1+u2) - v(u1) - v(u2) = 0 on 30-stage RC ladders (R 100, C 100 fF), 3 copies, relation nodes at stages 10 and 30
* bench.probes: v(r_10), v(r_30), v(f10), v(f30)
* bench.tol.norm: 5e-3
* bench.floor: 5e-11
* bench.expect: match
* bench.size: 107
* bench.tags: linear, tran, rc_ladder, relation, twin_block, reference_free, superposition
* bench.est_rss_mib: 16
* bench.note: Copy f is driven by u1 + u2 (PULSE(0 1 0.2n 0.1n 0.1n 1.5n 3n) in series with SIN(0 0.5 1.3G)), copy a by u1 alone, copy b by u2 alone; r_x(t) = 1 + (v_f - v_a - v_b)(x, t) at stages 10 and 30 through non-loading VCCS cards, exactly 1 for a linear time-invariant system. RustSpice (fixed step, UnrolledSymbolic, 107 unknowns, 501 rows): max |r-1| = 1.6e-15 (stage 10) / 1.8e-15 (stage 30); ngspice reference (10029 rows, tmax = tstep/20) 3.6e-15 / 3.3e-15. The relation nodes sit at 1 V (non-vacuous); their gate is tol.norm 5e-3 x floor 5e-11 = 2.5e-13 V absolute (140x above RustSpice). The waveform probes v(f10), v(f30) are normalised by their own swing (0.706 V, 0.402 V): RustSpice vs ngspice reference 2.6e-4 / 9.0e-5 of swing at tstep 10 ps (tol.norm 5e-3), so a solution driven to zero cannot satisfy the relation.
vf1 fa 0 PULSE(0 1 0.2n 0.1n 0.1n 1.5n 3n)
vf2 f0 fa SIN(0 0.5 1.3G)
rf1 f0 f1 100
cf1 f1 0 100f
rf2 f1 f2 100
cf2 f2 0 100f
rf3 f2 f3 100
cf3 f3 0 100f
rf4 f3 f4 100
cf4 f4 0 100f
rf5 f4 f5 100
cf5 f5 0 100f
rf6 f5 f6 100
cf6 f6 0 100f
rf7 f6 f7 100
cf7 f7 0 100f
rf8 f7 f8 100
cf8 f8 0 100f
rf9 f8 f9 100
cf9 f9 0 100f
rf10 f9 f10 100
cf10 f10 0 100f
rf11 f10 f11 100
cf11 f11 0 100f
rf12 f11 f12 100
cf12 f12 0 100f
rf13 f12 f13 100
cf13 f13 0 100f
rf14 f13 f14 100
cf14 f14 0 100f
rf15 f14 f15 100
cf15 f15 0 100f
rf16 f15 f16 100
cf16 f16 0 100f
rf17 f16 f17 100
cf17 f17 0 100f
rf18 f17 f18 100
cf18 f18 0 100f
rf19 f18 f19 100
cf19 f19 0 100f
rf20 f19 f20 100
cf20 f20 0 100f
rf21 f20 f21 100
cf21 f21 0 100f
rf22 f21 f22 100
cf22 f22 0 100f
rf23 f22 f23 100
cf23 f23 0 100f
rf24 f23 f24 100
cf24 f24 0 100f
rf25 f24 f25 100
cf25 f25 0 100f
rf26 f25 f26 100
cf26 f26 0 100f
rf27 f26 f27 100
cf27 f27 0 100f
rf28 f27 f28 100
cf28 f28 0 100f
rf29 f28 f29 100
cf29 f29 0 100f
rf30 f29 f30 100
cf30 f30 0 100f
va1 aa 0 PULSE(0 1 0.2n 0.1n 0.1n 1.5n 3n)
va2 a0 aa DC 0
ra1 a0 a1 100
ca1 a1 0 100f
ra2 a1 a2 100
ca2 a2 0 100f
ra3 a2 a3 100
ca3 a3 0 100f
ra4 a3 a4 100
ca4 a4 0 100f
ra5 a4 a5 100
ca5 a5 0 100f
ra6 a5 a6 100
ca6 a6 0 100f
ra7 a6 a7 100
ca7 a7 0 100f
ra8 a7 a8 100
ca8 a8 0 100f
ra9 a8 a9 100
ca9 a9 0 100f
ra10 a9 a10 100
ca10 a10 0 100f
ra11 a10 a11 100
ca11 a11 0 100f
ra12 a11 a12 100
ca12 a12 0 100f
ra13 a12 a13 100
ca13 a13 0 100f
ra14 a13 a14 100
ca14 a14 0 100f
ra15 a14 a15 100
ca15 a15 0 100f
ra16 a15 a16 100
ca16 a16 0 100f
ra17 a16 a17 100
ca17 a17 0 100f
ra18 a17 a18 100
ca18 a18 0 100f
ra19 a18 a19 100
ca19 a19 0 100f
ra20 a19 a20 100
ca20 a20 0 100f
ra21 a20 a21 100
ca21 a21 0 100f
ra22 a21 a22 100
ca22 a22 0 100f
ra23 a22 a23 100
ca23 a23 0 100f
ra24 a23 a24 100
ca24 a24 0 100f
ra25 a24 a25 100
ca25 a25 0 100f
ra26 a25 a26 100
ca26 a26 0 100f
ra27 a26 a27 100
ca27 a27 0 100f
ra28 a27 a28 100
ca28 a28 0 100f
ra29 a28 a29 100
ca29 a29 0 100f
ra30 a29 a30 100
ca30 a30 0 100f
vb1 ba 0 DC 0
vb2 b0 ba SIN(0 0.5 1.3G)
rb1 b0 b1 100
cb1 b1 0 100f
rb2 b1 b2 100
cb2 b2 0 100f
rb3 b2 b3 100
cb3 b3 0 100f
rb4 b3 b4 100
cb4 b4 0 100f
rb5 b4 b5 100
cb5 b5 0 100f
rb6 b5 b6 100
cb6 b6 0 100f
rb7 b6 b7 100
cb7 b7 0 100f
rb8 b7 b8 100
cb8 b8 0 100f
rb9 b8 b9 100
cb9 b9 0 100f
rb10 b9 b10 100
cb10 b10 0 100f
rb11 b10 b11 100
cb11 b11 0 100f
rb12 b11 b12 100
cb12 b12 0 100f
rb13 b12 b13 100
cb13 b13 0 100f
rb14 b13 b14 100
cb14 b14 0 100f
rb15 b14 b15 100
cb15 b15 0 100f
rb16 b15 b16 100
cb16 b16 0 100f
rb17 b16 b17 100
cb17 b17 0 100f
rb18 b17 b18 100
cb18 b18 0 100f
rb19 b18 b19 100
cb19 b19 0 100f
rb20 b19 b20 100
cb20 b20 0 100f
rb21 b20 b21 100
cb21 b21 0 100f
rb22 b21 b22 100
cb22 b22 0 100f
rb23 b22 b23 100
cb23 b23 0 100f
rb24 b23 b24 100
cb24 b24 0 100f
rb25 b24 b25 100
cb25 b25 0 100f
rb26 b25 b26 100
cb26 b26 0 100f
rb27 b26 b27 100
cb27 b27 0 100f
rb28 b27 b28 100
cb28 b28 0 100f
rb29 b28 b29 100
cb29 b29 0 100f
rb30 b29 b30 100
cb30 b30 0 100f
vref ref1 0 DC 1
rm10 r_10 ref1 1
gf10 0 r_10 f10 0 1
ga10 r_10 0 a10 0 1
gb10 r_10 0 b10 0 1
rm30 r_30 ref1 1
gf30 0 r_30 f30 0 1
ga30 r_30 0 a30 0 1
gb30 r_30 0 b30 0 1
.tran 10p 5n
.end
