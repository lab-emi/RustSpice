.ic without uic on a driven node
* bench.id: cap_lang_ic_without_uic
* bench.group: capability
* bench.title: .ic v(out)=5 without uic: the DC solution is computed with the node forced, then released (V1 = 1 V, tau = 1 ms)
* bench.probes: v(out)
* bench.tags: linear, tran, ic
* bench.note: only v(out) is probed; the t = 0 branch current of V1 reflects the forced node and differs between simulators
V1 in 0 DC 1
R1 in out 1k
C1 out 0 1u
.ic v(out)=5
.tran 20u 3m
.end
