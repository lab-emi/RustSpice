Full-wave diode bridge rectifier
* bench.id: acc_tr_diode_bridge
* bench.group: accuracy
* bench.title: Full-wave bridge (10 V, 1 kHz, floating source) into 22 uF / 1 k, 100 steps per period
* bench.probes: v(p)
* bench.tol.norm: 5e-3
* bench.ref: reltol=1e-5 abstol=1e-12 vntol=1e-6
* bench.ref.tmax: 0.5u
* bench.tags: nonlinear, tran, diode, rectifier, floating
* bench.est_rss_mib: 16
* bench.note: ngspice cannot converge this floating bridge at reltol=1e-6 ("Timestep too small"); the reference uses reltol=1e-5 abstol=1e-12 vntol=1e-6 with tmax=tstep/20 (checked against tmax=tstep/80: agree to 4e-6). tol.norm was 2e-2 and is tightened to 5e-3 (the default, SUITE_SPEC 13.4.2): RustSpice with the clamp out of the way is at 2.8e-3. Reference verified: the reference run agrees with an independent adaptive Radau IIA integration to 3.2e-5 of the swing (ngspice as-is: 5.5e-3 against the same integration).
VS a b SIN(0 10 1k)
RG b 0 1meg
D1 a p DM
D2 b p DM
D3 n a DM
D4 n b DM
C1 p n 22u
RL p n 1k
RN n 0 1meg
.model DM D(IS=1e-14 N=1.05)
.tran 10u 5m
.end
