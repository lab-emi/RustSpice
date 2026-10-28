NMOS common-source array 10x10 on resistive supply and gate grids, one operating point
* bench.id: perf_med_mos_array_op_10x10
* bench.group: performance
* bench.title: NMOS common-source array 10x10 on resistive supply and gate grids, one operating point
* bench.probes: v(d0_0), v(d5_5), v(d9_9), i(vdd)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 304
* bench.est_rss_mib: 10
* bench.tags: nonlinear, op, mos, array, mesh, medium
* bench.note: 100 loaded NMOS cells whose supply and gate nodes are coupled by two resistive grids (304 unknowns): MOS Jacobian stamps and 2-D fill together
.model nm NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
VDD vdd 0 3.3
VG gin 0 1.3
Rvd vdd p0_0 5
Rvg gin g0_0 50
RD0_0 p0_0 d0_0 5k
M0_0 d0_0 g0_0 0 0 nm W=10u L=1u
Rpv0_0 p0_0 p0_1 20
Rgv0_0 g0_0 g0_1 200
Rph0_0 p0_0 p1_0 20
Rgh0_0 g0_0 g1_0 200
RD0_1 p0_1 d0_1 5k
M0_1 d0_1 g0_1 0 0 nm W=10u L=1u
Rpv0_1 p0_1 p0_2 20
Rgv0_1 g0_1 g0_2 200
Rph0_1 p0_1 p1_1 20
Rgh0_1 g0_1 g1_1 200
RD0_2 p0_2 d0_2 5k
M0_2 d0_2 g0_2 0 0 nm W=10u L=1u
Rpv0_2 p0_2 p0_3 20
Rgv0_2 g0_2 g0_3 200
Rph0_2 p0_2 p1_2 20
Rgh0_2 g0_2 g1_2 200
RD0_3 p0_3 d0_3 5k
M0_3 d0_3 g0_3 0 0 nm W=10u L=1u
Rpv0_3 p0_3 p0_4 20
Rgv0_3 g0_3 g0_4 200
Rph0_3 p0_3 p1_3 20
Rgh0_3 g0_3 g1_3 200
RD0_4 p0_4 d0_4 5k
M0_4 d0_4 g0_4 0 0 nm W=10u L=1u
Rpv0_4 p0_4 p0_5 20
Rgv0_4 g0_4 g0_5 200
Rph0_4 p0_4 p1_4 20
Rgh0_4 g0_4 g1_4 200
RD0_5 p0_5 d0_5 5k
M0_5 d0_5 g0_5 0 0 nm W=10u L=1u
Rpv0_5 p0_5 p0_6 20
Rgv0_5 g0_5 g0_6 200
Rph0_5 p0_5 p1_5 20
Rgh0_5 g0_5 g1_5 200
RD0_6 p0_6 d0_6 5k
M0_6 d0_6 g0_6 0 0 nm W=10u L=1u
Rpv0_6 p0_6 p0_7 20
Rgv0_6 g0_6 g0_7 200
Rph0_6 p0_6 p1_6 20
Rgh0_6 g0_6 g1_6 200
RD0_7 p0_7 d0_7 5k
M0_7 d0_7 g0_7 0 0 nm W=10u L=1u
Rpv0_7 p0_7 p0_8 20
Rgv0_7 g0_7 g0_8 200
Rph0_7 p0_7 p1_7 20
Rgh0_7 g0_7 g1_7 200
RD0_8 p0_8 d0_8 5k
M0_8 d0_8 g0_8 0 0 nm W=10u L=1u
Rpv0_8 p0_8 p0_9 20
Rgv0_8 g0_8 g0_9 200
Rph0_8 p0_8 p1_8 20
Rgh0_8 g0_8 g1_8 200
RD0_9 p0_9 d0_9 5k
M0_9 d0_9 g0_9 0 0 nm W=10u L=1u
Rph0_9 p0_9 p1_9 20
Rgh0_9 g0_9 g1_9 200
RD1_0 p1_0 d1_0 5k
M1_0 d1_0 g1_0 0 0 nm W=10u L=1u
Rpv1_0 p1_0 p1_1 20
Rgv1_0 g1_0 g1_1 200
Rph1_0 p1_0 p2_0 20
Rgh1_0 g1_0 g2_0 200
RD1_1 p1_1 d1_1 5k
M1_1 d1_1 g1_1 0 0 nm W=10u L=1u
Rpv1_1 p1_1 p1_2 20
Rgv1_1 g1_1 g1_2 200
Rph1_1 p1_1 p2_1 20
Rgh1_1 g1_1 g2_1 200
RD1_2 p1_2 d1_2 5k
M1_2 d1_2 g1_2 0 0 nm W=10u L=1u
Rpv1_2 p1_2 p1_3 20
Rgv1_2 g1_2 g1_3 200
Rph1_2 p1_2 p2_2 20
Rgh1_2 g1_2 g2_2 200
RD1_3 p1_3 d1_3 5k
M1_3 d1_3 g1_3 0 0 nm W=10u L=1u
Rpv1_3 p1_3 p1_4 20
Rgv1_3 g1_3 g1_4 200
Rph1_3 p1_3 p2_3 20
Rgh1_3 g1_3 g2_3 200
RD1_4 p1_4 d1_4 5k
M1_4 d1_4 g1_4 0 0 nm W=10u L=1u
Rpv1_4 p1_4 p1_5 20
Rgv1_4 g1_4 g1_5 200
Rph1_4 p1_4 p2_4 20
Rgh1_4 g1_4 g2_4 200
RD1_5 p1_5 d1_5 5k
M1_5 d1_5 g1_5 0 0 nm W=10u L=1u
Rpv1_5 p1_5 p1_6 20
Rgv1_5 g1_5 g1_6 200
Rph1_5 p1_5 p2_5 20
Rgh1_5 g1_5 g2_5 200
RD1_6 p1_6 d1_6 5k
M1_6 d1_6 g1_6 0 0 nm W=10u L=1u
Rpv1_6 p1_6 p1_7 20
Rgv1_6 g1_6 g1_7 200
Rph1_6 p1_6 p2_6 20
Rgh1_6 g1_6 g2_6 200
RD1_7 p1_7 d1_7 5k
M1_7 d1_7 g1_7 0 0 nm W=10u L=1u
Rpv1_7 p1_7 p1_8 20
Rgv1_7 g1_7 g1_8 200
Rph1_7 p1_7 p2_7 20
Rgh1_7 g1_7 g2_7 200
RD1_8 p1_8 d1_8 5k
M1_8 d1_8 g1_8 0 0 nm W=10u L=1u
Rpv1_8 p1_8 p1_9 20
Rgv1_8 g1_8 g1_9 200
Rph1_8 p1_8 p2_8 20
Rgh1_8 g1_8 g2_8 200
RD1_9 p1_9 d1_9 5k
M1_9 d1_9 g1_9 0 0 nm W=10u L=1u
Rph1_9 p1_9 p2_9 20
Rgh1_9 g1_9 g2_9 200
RD2_0 p2_0 d2_0 5k
M2_0 d2_0 g2_0 0 0 nm W=10u L=1u
Rpv2_0 p2_0 p2_1 20
Rgv2_0 g2_0 g2_1 200
Rph2_0 p2_0 p3_0 20
Rgh2_0 g2_0 g3_0 200
RD2_1 p2_1 d2_1 5k
M2_1 d2_1 g2_1 0 0 nm W=10u L=1u
Rpv2_1 p2_1 p2_2 20
Rgv2_1 g2_1 g2_2 200
Rph2_1 p2_1 p3_1 20
Rgh2_1 g2_1 g3_1 200
RD2_2 p2_2 d2_2 5k
M2_2 d2_2 g2_2 0 0 nm W=10u L=1u
Rpv2_2 p2_2 p2_3 20
Rgv2_2 g2_2 g2_3 200
Rph2_2 p2_2 p3_2 20
Rgh2_2 g2_2 g3_2 200
RD2_3 p2_3 d2_3 5k
M2_3 d2_3 g2_3 0 0 nm W=10u L=1u
Rpv2_3 p2_3 p2_4 20
Rgv2_3 g2_3 g2_4 200
Rph2_3 p2_3 p3_3 20
Rgh2_3 g2_3 g3_3 200
RD2_4 p2_4 d2_4 5k
M2_4 d2_4 g2_4 0 0 nm W=10u L=1u
Rpv2_4 p2_4 p2_5 20
Rgv2_4 g2_4 g2_5 200
Rph2_4 p2_4 p3_4 20
Rgh2_4 g2_4 g3_4 200
RD2_5 p2_5 d2_5 5k
M2_5 d2_5 g2_5 0 0 nm W=10u L=1u
Rpv2_5 p2_5 p2_6 20
Rgv2_5 g2_5 g2_6 200
Rph2_5 p2_5 p3_5 20
Rgh2_5 g2_5 g3_5 200
RD2_6 p2_6 d2_6 5k
M2_6 d2_6 g2_6 0 0 nm W=10u L=1u
Rpv2_6 p2_6 p2_7 20
Rgv2_6 g2_6 g2_7 200
Rph2_6 p2_6 p3_6 20
Rgh2_6 g2_6 g3_6 200
RD2_7 p2_7 d2_7 5k
M2_7 d2_7 g2_7 0 0 nm W=10u L=1u
Rpv2_7 p2_7 p2_8 20
Rgv2_7 g2_7 g2_8 200
Rph2_7 p2_7 p3_7 20
Rgh2_7 g2_7 g3_7 200
RD2_8 p2_8 d2_8 5k
M2_8 d2_8 g2_8 0 0 nm W=10u L=1u
Rpv2_8 p2_8 p2_9 20
Rgv2_8 g2_8 g2_9 200
Rph2_8 p2_8 p3_8 20
Rgh2_8 g2_8 g3_8 200
RD2_9 p2_9 d2_9 5k
M2_9 d2_9 g2_9 0 0 nm W=10u L=1u
Rph2_9 p2_9 p3_9 20
Rgh2_9 g2_9 g3_9 200
RD3_0 p3_0 d3_0 5k
M3_0 d3_0 g3_0 0 0 nm W=10u L=1u
Rpv3_0 p3_0 p3_1 20
Rgv3_0 g3_0 g3_1 200
Rph3_0 p3_0 p4_0 20
Rgh3_0 g3_0 g4_0 200
RD3_1 p3_1 d3_1 5k
M3_1 d3_1 g3_1 0 0 nm W=10u L=1u
Rpv3_1 p3_1 p3_2 20
Rgv3_1 g3_1 g3_2 200
Rph3_1 p3_1 p4_1 20
Rgh3_1 g3_1 g4_1 200
RD3_2 p3_2 d3_2 5k
M3_2 d3_2 g3_2 0 0 nm W=10u L=1u
Rpv3_2 p3_2 p3_3 20
Rgv3_2 g3_2 g3_3 200
Rph3_2 p3_2 p4_2 20
Rgh3_2 g3_2 g4_2 200
RD3_3 p3_3 d3_3 5k
M3_3 d3_3 g3_3 0 0 nm W=10u L=1u
Rpv3_3 p3_3 p3_4 20
Rgv3_3 g3_3 g3_4 200
Rph3_3 p3_3 p4_3 20
Rgh3_3 g3_3 g4_3 200
RD3_4 p3_4 d3_4 5k
M3_4 d3_4 g3_4 0 0 nm W=10u L=1u
Rpv3_4 p3_4 p3_5 20
Rgv3_4 g3_4 g3_5 200
Rph3_4 p3_4 p4_4 20
Rgh3_4 g3_4 g4_4 200
RD3_5 p3_5 d3_5 5k
M3_5 d3_5 g3_5 0 0 nm W=10u L=1u
Rpv3_5 p3_5 p3_6 20
Rgv3_5 g3_5 g3_6 200
Rph3_5 p3_5 p4_5 20
Rgh3_5 g3_5 g4_5 200
RD3_6 p3_6 d3_6 5k
M3_6 d3_6 g3_6 0 0 nm W=10u L=1u
Rpv3_6 p3_6 p3_7 20
Rgv3_6 g3_6 g3_7 200
Rph3_6 p3_6 p4_6 20
Rgh3_6 g3_6 g4_6 200
RD3_7 p3_7 d3_7 5k
M3_7 d3_7 g3_7 0 0 nm W=10u L=1u
Rpv3_7 p3_7 p3_8 20
Rgv3_7 g3_7 g3_8 200
Rph3_7 p3_7 p4_7 20
Rgh3_7 g3_7 g4_7 200
RD3_8 p3_8 d3_8 5k
M3_8 d3_8 g3_8 0 0 nm W=10u L=1u
Rpv3_8 p3_8 p3_9 20
Rgv3_8 g3_8 g3_9 200
Rph3_8 p3_8 p4_8 20
Rgh3_8 g3_8 g4_8 200
RD3_9 p3_9 d3_9 5k
M3_9 d3_9 g3_9 0 0 nm W=10u L=1u
Rph3_9 p3_9 p4_9 20
Rgh3_9 g3_9 g4_9 200
RD4_0 p4_0 d4_0 5k
M4_0 d4_0 g4_0 0 0 nm W=10u L=1u
Rpv4_0 p4_0 p4_1 20
Rgv4_0 g4_0 g4_1 200
Rph4_0 p4_0 p5_0 20
Rgh4_0 g4_0 g5_0 200
RD4_1 p4_1 d4_1 5k
M4_1 d4_1 g4_1 0 0 nm W=10u L=1u
Rpv4_1 p4_1 p4_2 20
Rgv4_1 g4_1 g4_2 200
Rph4_1 p4_1 p5_1 20
Rgh4_1 g4_1 g5_1 200
RD4_2 p4_2 d4_2 5k
M4_2 d4_2 g4_2 0 0 nm W=10u L=1u
Rpv4_2 p4_2 p4_3 20
Rgv4_2 g4_2 g4_3 200
Rph4_2 p4_2 p5_2 20
Rgh4_2 g4_2 g5_2 200
RD4_3 p4_3 d4_3 5k
M4_3 d4_3 g4_3 0 0 nm W=10u L=1u
Rpv4_3 p4_3 p4_4 20
Rgv4_3 g4_3 g4_4 200
Rph4_3 p4_3 p5_3 20
Rgh4_3 g4_3 g5_3 200
RD4_4 p4_4 d4_4 5k
M4_4 d4_4 g4_4 0 0 nm W=10u L=1u
Rpv4_4 p4_4 p4_5 20
Rgv4_4 g4_4 g4_5 200
Rph4_4 p4_4 p5_4 20
Rgh4_4 g4_4 g5_4 200
RD4_5 p4_5 d4_5 5k
M4_5 d4_5 g4_5 0 0 nm W=10u L=1u
Rpv4_5 p4_5 p4_6 20
Rgv4_5 g4_5 g4_6 200
Rph4_5 p4_5 p5_5 20
Rgh4_5 g4_5 g5_5 200
RD4_6 p4_6 d4_6 5k
M4_6 d4_6 g4_6 0 0 nm W=10u L=1u
Rpv4_6 p4_6 p4_7 20
Rgv4_6 g4_6 g4_7 200
Rph4_6 p4_6 p5_6 20
Rgh4_6 g4_6 g5_6 200
RD4_7 p4_7 d4_7 5k
M4_7 d4_7 g4_7 0 0 nm W=10u L=1u
Rpv4_7 p4_7 p4_8 20
Rgv4_7 g4_7 g4_8 200
Rph4_7 p4_7 p5_7 20
Rgh4_7 g4_7 g5_7 200
RD4_8 p4_8 d4_8 5k
M4_8 d4_8 g4_8 0 0 nm W=10u L=1u
Rpv4_8 p4_8 p4_9 20
Rgv4_8 g4_8 g4_9 200
Rph4_8 p4_8 p5_8 20
Rgh4_8 g4_8 g5_8 200
RD4_9 p4_9 d4_9 5k
M4_9 d4_9 g4_9 0 0 nm W=10u L=1u
Rph4_9 p4_9 p5_9 20
Rgh4_9 g4_9 g5_9 200
RD5_0 p5_0 d5_0 5k
M5_0 d5_0 g5_0 0 0 nm W=10u L=1u
Rpv5_0 p5_0 p5_1 20
Rgv5_0 g5_0 g5_1 200
Rph5_0 p5_0 p6_0 20
Rgh5_0 g5_0 g6_0 200
RD5_1 p5_1 d5_1 5k
M5_1 d5_1 g5_1 0 0 nm W=10u L=1u
Rpv5_1 p5_1 p5_2 20
Rgv5_1 g5_1 g5_2 200
Rph5_1 p5_1 p6_1 20
Rgh5_1 g5_1 g6_1 200
RD5_2 p5_2 d5_2 5k
M5_2 d5_2 g5_2 0 0 nm W=10u L=1u
Rpv5_2 p5_2 p5_3 20
Rgv5_2 g5_2 g5_3 200
Rph5_2 p5_2 p6_2 20
Rgh5_2 g5_2 g6_2 200
RD5_3 p5_3 d5_3 5k
M5_3 d5_3 g5_3 0 0 nm W=10u L=1u
Rpv5_3 p5_3 p5_4 20
Rgv5_3 g5_3 g5_4 200
Rph5_3 p5_3 p6_3 20
Rgh5_3 g5_3 g6_3 200
RD5_4 p5_4 d5_4 5k
M5_4 d5_4 g5_4 0 0 nm W=10u L=1u
Rpv5_4 p5_4 p5_5 20
Rgv5_4 g5_4 g5_5 200
Rph5_4 p5_4 p6_4 20
Rgh5_4 g5_4 g6_4 200
RD5_5 p5_5 d5_5 5k
M5_5 d5_5 g5_5 0 0 nm W=10u L=1u
Rpv5_5 p5_5 p5_6 20
Rgv5_5 g5_5 g5_6 200
Rph5_5 p5_5 p6_5 20
Rgh5_5 g5_5 g6_5 200
RD5_6 p5_6 d5_6 5k
M5_6 d5_6 g5_6 0 0 nm W=10u L=1u
Rpv5_6 p5_6 p5_7 20
Rgv5_6 g5_6 g5_7 200
Rph5_6 p5_6 p6_6 20
Rgh5_6 g5_6 g6_6 200
RD5_7 p5_7 d5_7 5k
M5_7 d5_7 g5_7 0 0 nm W=10u L=1u
Rpv5_7 p5_7 p5_8 20
Rgv5_7 g5_7 g5_8 200
Rph5_7 p5_7 p6_7 20
Rgh5_7 g5_7 g6_7 200
RD5_8 p5_8 d5_8 5k
M5_8 d5_8 g5_8 0 0 nm W=10u L=1u
Rpv5_8 p5_8 p5_9 20
Rgv5_8 g5_8 g5_9 200
Rph5_8 p5_8 p6_8 20
Rgh5_8 g5_8 g6_8 200
RD5_9 p5_9 d5_9 5k
M5_9 d5_9 g5_9 0 0 nm W=10u L=1u
Rph5_9 p5_9 p6_9 20
Rgh5_9 g5_9 g6_9 200
RD6_0 p6_0 d6_0 5k
M6_0 d6_0 g6_0 0 0 nm W=10u L=1u
Rpv6_0 p6_0 p6_1 20
Rgv6_0 g6_0 g6_1 200
Rph6_0 p6_0 p7_0 20
Rgh6_0 g6_0 g7_0 200
RD6_1 p6_1 d6_1 5k
M6_1 d6_1 g6_1 0 0 nm W=10u L=1u
Rpv6_1 p6_1 p6_2 20
Rgv6_1 g6_1 g6_2 200
Rph6_1 p6_1 p7_1 20
Rgh6_1 g6_1 g7_1 200
RD6_2 p6_2 d6_2 5k
M6_2 d6_2 g6_2 0 0 nm W=10u L=1u
Rpv6_2 p6_2 p6_3 20
Rgv6_2 g6_2 g6_3 200
Rph6_2 p6_2 p7_2 20
Rgh6_2 g6_2 g7_2 200
RD6_3 p6_3 d6_3 5k
M6_3 d6_3 g6_3 0 0 nm W=10u L=1u
Rpv6_3 p6_3 p6_4 20
Rgv6_3 g6_3 g6_4 200
Rph6_3 p6_3 p7_3 20
Rgh6_3 g6_3 g7_3 200
RD6_4 p6_4 d6_4 5k
M6_4 d6_4 g6_4 0 0 nm W=10u L=1u
Rpv6_4 p6_4 p6_5 20
Rgv6_4 g6_4 g6_5 200
Rph6_4 p6_4 p7_4 20
Rgh6_4 g6_4 g7_4 200
RD6_5 p6_5 d6_5 5k
M6_5 d6_5 g6_5 0 0 nm W=10u L=1u
Rpv6_5 p6_5 p6_6 20
Rgv6_5 g6_5 g6_6 200
Rph6_5 p6_5 p7_5 20
Rgh6_5 g6_5 g7_5 200
RD6_6 p6_6 d6_6 5k
M6_6 d6_6 g6_6 0 0 nm W=10u L=1u
Rpv6_6 p6_6 p6_7 20
Rgv6_6 g6_6 g6_7 200
Rph6_6 p6_6 p7_6 20
Rgh6_6 g6_6 g7_6 200
RD6_7 p6_7 d6_7 5k
M6_7 d6_7 g6_7 0 0 nm W=10u L=1u
Rpv6_7 p6_7 p6_8 20
Rgv6_7 g6_7 g6_8 200
Rph6_7 p6_7 p7_7 20
Rgh6_7 g6_7 g7_7 200
RD6_8 p6_8 d6_8 5k
M6_8 d6_8 g6_8 0 0 nm W=10u L=1u
Rpv6_8 p6_8 p6_9 20
Rgv6_8 g6_8 g6_9 200
Rph6_8 p6_8 p7_8 20
Rgh6_8 g6_8 g7_8 200
RD6_9 p6_9 d6_9 5k
M6_9 d6_9 g6_9 0 0 nm W=10u L=1u
Rph6_9 p6_9 p7_9 20
Rgh6_9 g6_9 g7_9 200
RD7_0 p7_0 d7_0 5k
M7_0 d7_0 g7_0 0 0 nm W=10u L=1u
Rpv7_0 p7_0 p7_1 20
Rgv7_0 g7_0 g7_1 200
Rph7_0 p7_0 p8_0 20
Rgh7_0 g7_0 g8_0 200
RD7_1 p7_1 d7_1 5k
M7_1 d7_1 g7_1 0 0 nm W=10u L=1u
Rpv7_1 p7_1 p7_2 20
Rgv7_1 g7_1 g7_2 200
Rph7_1 p7_1 p8_1 20
Rgh7_1 g7_1 g8_1 200
RD7_2 p7_2 d7_2 5k
M7_2 d7_2 g7_2 0 0 nm W=10u L=1u
Rpv7_2 p7_2 p7_3 20
Rgv7_2 g7_2 g7_3 200
Rph7_2 p7_2 p8_2 20
Rgh7_2 g7_2 g8_2 200
RD7_3 p7_3 d7_3 5k
M7_3 d7_3 g7_3 0 0 nm W=10u L=1u
Rpv7_3 p7_3 p7_4 20
Rgv7_3 g7_3 g7_4 200
Rph7_3 p7_3 p8_3 20
Rgh7_3 g7_3 g8_3 200
RD7_4 p7_4 d7_4 5k
M7_4 d7_4 g7_4 0 0 nm W=10u L=1u
Rpv7_4 p7_4 p7_5 20
Rgv7_4 g7_4 g7_5 200
Rph7_4 p7_4 p8_4 20
Rgh7_4 g7_4 g8_4 200
RD7_5 p7_5 d7_5 5k
M7_5 d7_5 g7_5 0 0 nm W=10u L=1u
Rpv7_5 p7_5 p7_6 20
Rgv7_5 g7_5 g7_6 200
Rph7_5 p7_5 p8_5 20
Rgh7_5 g7_5 g8_5 200
RD7_6 p7_6 d7_6 5k
M7_6 d7_6 g7_6 0 0 nm W=10u L=1u
Rpv7_6 p7_6 p7_7 20
Rgv7_6 g7_6 g7_7 200
Rph7_6 p7_6 p8_6 20
Rgh7_6 g7_6 g8_6 200
RD7_7 p7_7 d7_7 5k
M7_7 d7_7 g7_7 0 0 nm W=10u L=1u
Rpv7_7 p7_7 p7_8 20
Rgv7_7 g7_7 g7_8 200
Rph7_7 p7_7 p8_7 20
Rgh7_7 g7_7 g8_7 200
RD7_8 p7_8 d7_8 5k
M7_8 d7_8 g7_8 0 0 nm W=10u L=1u
Rpv7_8 p7_8 p7_9 20
Rgv7_8 g7_8 g7_9 200
Rph7_8 p7_8 p8_8 20
Rgh7_8 g7_8 g8_8 200
RD7_9 p7_9 d7_9 5k
M7_9 d7_9 g7_9 0 0 nm W=10u L=1u
Rph7_9 p7_9 p8_9 20
Rgh7_9 g7_9 g8_9 200
RD8_0 p8_0 d8_0 5k
M8_0 d8_0 g8_0 0 0 nm W=10u L=1u
Rpv8_0 p8_0 p8_1 20
Rgv8_0 g8_0 g8_1 200
Rph8_0 p8_0 p9_0 20
Rgh8_0 g8_0 g9_0 200
RD8_1 p8_1 d8_1 5k
M8_1 d8_1 g8_1 0 0 nm W=10u L=1u
Rpv8_1 p8_1 p8_2 20
Rgv8_1 g8_1 g8_2 200
Rph8_1 p8_1 p9_1 20
Rgh8_1 g8_1 g9_1 200
RD8_2 p8_2 d8_2 5k
M8_2 d8_2 g8_2 0 0 nm W=10u L=1u
Rpv8_2 p8_2 p8_3 20
Rgv8_2 g8_2 g8_3 200
Rph8_2 p8_2 p9_2 20
Rgh8_2 g8_2 g9_2 200
RD8_3 p8_3 d8_3 5k
M8_3 d8_3 g8_3 0 0 nm W=10u L=1u
Rpv8_3 p8_3 p8_4 20
Rgv8_3 g8_3 g8_4 200
Rph8_3 p8_3 p9_3 20
Rgh8_3 g8_3 g9_3 200
RD8_4 p8_4 d8_4 5k
M8_4 d8_4 g8_4 0 0 nm W=10u L=1u
Rpv8_4 p8_4 p8_5 20
Rgv8_4 g8_4 g8_5 200
Rph8_4 p8_4 p9_4 20
Rgh8_4 g8_4 g9_4 200
RD8_5 p8_5 d8_5 5k
M8_5 d8_5 g8_5 0 0 nm W=10u L=1u
Rpv8_5 p8_5 p8_6 20
Rgv8_5 g8_5 g8_6 200
Rph8_5 p8_5 p9_5 20
Rgh8_5 g8_5 g9_5 200
RD8_6 p8_6 d8_6 5k
M8_6 d8_6 g8_6 0 0 nm W=10u L=1u
Rpv8_6 p8_6 p8_7 20
Rgv8_6 g8_6 g8_7 200
Rph8_6 p8_6 p9_6 20
Rgh8_6 g8_6 g9_6 200
RD8_7 p8_7 d8_7 5k
M8_7 d8_7 g8_7 0 0 nm W=10u L=1u
Rpv8_7 p8_7 p8_8 20
Rgv8_7 g8_7 g8_8 200
Rph8_7 p8_7 p9_7 20
Rgh8_7 g8_7 g9_7 200
RD8_8 p8_8 d8_8 5k
M8_8 d8_8 g8_8 0 0 nm W=10u L=1u
Rpv8_8 p8_8 p8_9 20
Rgv8_8 g8_8 g8_9 200
Rph8_8 p8_8 p9_8 20
Rgh8_8 g8_8 g9_8 200
RD8_9 p8_9 d8_9 5k
M8_9 d8_9 g8_9 0 0 nm W=10u L=1u
Rph8_9 p8_9 p9_9 20
Rgh8_9 g8_9 g9_9 200
RD9_0 p9_0 d9_0 5k
M9_0 d9_0 g9_0 0 0 nm W=10u L=1u
Rpv9_0 p9_0 p9_1 20
Rgv9_0 g9_0 g9_1 200
RD9_1 p9_1 d9_1 5k
M9_1 d9_1 g9_1 0 0 nm W=10u L=1u
Rpv9_1 p9_1 p9_2 20
Rgv9_1 g9_1 g9_2 200
RD9_2 p9_2 d9_2 5k
M9_2 d9_2 g9_2 0 0 nm W=10u L=1u
Rpv9_2 p9_2 p9_3 20
Rgv9_2 g9_2 g9_3 200
RD9_3 p9_3 d9_3 5k
M9_3 d9_3 g9_3 0 0 nm W=10u L=1u
Rpv9_3 p9_3 p9_4 20
Rgv9_3 g9_3 g9_4 200
RD9_4 p9_4 d9_4 5k
M9_4 d9_4 g9_4 0 0 nm W=10u L=1u
Rpv9_4 p9_4 p9_5 20
Rgv9_4 g9_4 g9_5 200
RD9_5 p9_5 d9_5 5k
M9_5 d9_5 g9_5 0 0 nm W=10u L=1u
Rpv9_5 p9_5 p9_6 20
Rgv9_5 g9_5 g9_6 200
RD9_6 p9_6 d9_6 5k
M9_6 d9_6 g9_6 0 0 nm W=10u L=1u
Rpv9_6 p9_6 p9_7 20
Rgv9_6 g9_6 g9_7 200
RD9_7 p9_7 d9_7 5k
M9_7 d9_7 g9_7 0 0 nm W=10u L=1u
Rpv9_7 p9_7 p9_8 20
Rgv9_7 g9_7 g9_8 200
RD9_8 p9_8 d9_8 5k
M9_8 d9_8 g9_8 0 0 nm W=10u L=1u
Rpv9_8 p9_8 p9_9 20
Rgv9_8 g9_8 g9_9 200
RD9_9 p9_9 d9_9 5k
M9_9 d9_9 g9_9 0 0 nm W=10u L=1u
.op
.end
