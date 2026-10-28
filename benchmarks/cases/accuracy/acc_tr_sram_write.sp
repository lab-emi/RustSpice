6T SRAM-like cell write
* bench.id: acc_tr_sram_write
* bench.group: accuracy
* bench.title: Cross-coupled inverter cell (stored 0) overwritten with 1 through two NMOS access devices
* bench.probes: v(q), v(qb)
* bench.tol.norm: 5e-3
* bench.tags: nonlinear, tran, mos, bistable
* bench.est_rss_mib: 16
VDD vdd 0 DC 3.3
MPL q qb vdd vdd PMOD W=2u L=1u
MNL q qb 0 0 NMOD W=4u L=1u
MPR qb q vdd vdd PMOD W=2u L=1u
MNR qb q 0 0 NMOD W=4u L=1u
MAL bl wl q 0 NMOD W=3u L=1u
MAR blb wl qb 0 NMOD W=3u L=1u
Cq q 0 5f
Cqb qb 0 5f
Rt1 q 0 1Meg
Rt2 qb vdd 1Meg
VBLH blh 0 DC 3.3
RBL blh bl 1k
CBL bl 0 20f
RBLB blb 0 1k
CBLB blb 0 20f
VWL wl 0 PULSE(0 3.3 0.5n 0.1n 0.1n 1n 10n)
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 5p 4n
.end
