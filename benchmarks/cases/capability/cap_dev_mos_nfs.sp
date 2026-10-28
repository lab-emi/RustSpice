NMOS L1 with NFS=1e11 (surface states; no effect in level 1)
* bench.id: cap_dev_mos_nfs
* bench.group: capability
* bench.title: NMOS L1 with NFS=1e11 (surface states; no effect in level 1)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
VDS d 0 3
VG g 0 0
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u NFS=1e11 TOX=1e-8)
.dc VG 0 1.2 0.1
.end
