tran 1u 1n: one step spans the whole analysis
* bench.id: rob_hs_tran_step_greater_than_stop
* bench.group: robustness
* bench.title: .tran 1u 1n: one step spans the whole analysis
* bench.probes: v(2)
* bench.oracle: tran_lti
* bench.expect: match
* bench.known_gap: fixed-step-accuracy
* bench.gap.note: tstep > tstop makes RustSpice take one fixed backward-Euler step over the whole window: v(2)(1 ns) = 0.2939 V (32 % high) versus exact 0.22269 V; the same deck with tstep 0.05n gives 0.22292
* bench.tags: hostile, tran, args, silent_risk
* bench.est_rss_mib: 64
* bench.note: exact (ODE, rtol 1e-12) = 0.2226942 V; ngspice with reltol=1e-9 gives 0.2226937 V. needs the end point t=tstop in the comparison grid (the nominal tstep grid degenerates to t=0); smooth SIN drive, tau = 1 ns = tstop
V1 1 0 SIN(0 1 100meg)
R1 1 2 1k
C1 2 0 1p
.tran 1u 1n
.end
