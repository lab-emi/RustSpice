Lossless LC tank kicked by a current pulse, then free ring
* bench.id: acc_ord_lc_free_ring_energy_period
* bench.group: accuracy
* bench.title: Lossless LC tank (100 nH, 10 pF, f0 = 159 MHz) kicked by a 4 ns, 10 mA current pulse, then 5.7 free periods
* bench.probes: v(n), i(l1)
* bench.tol.norm: 8e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.series.tstep: 1n 500p 250p 125p 62.5p 31.25p
* bench.tags: linear, tran, lc, isrc, ringing, series, order, conservation
* bench.est_rss_mib: 16
* bench.note: Conservation and frequency deck on a closed lossless circuit; the existing acc_tr_lc_ring is step-driven, loosened to 1.2e-1 and has no energy or period oracle. Truth is scipy DOP853 rtol 1e-13 split at the PULSE breakpoints (self-check against Radau 1.7e-13 of the swing). After the source has ended (t >= 4 ns) RustSpice's relative energy drift is 1.6e-15..1.0e-12 over h = 1n..31.25p (ngspice as-is 7e-15..1e-12, reltol=1e-9 5e-12..1e-11: both conserve). The measured period error equals the closed-form trapezoid warping (w0 h/2)/atan(w0 h/2) - 1 to 7 digits (+7.8405e-2 at 1n, +8.137e-5 at 31.25p, slope 2.00) while ngspice with reltol=1e-9 is +2.9e-6 flat; the injected-energy error is -2.1e-1 .. -2.5e-4 (slope 2.00). tol.norm 8e-3 is 1.35x RustSpice's 5.9e-3 at the nominal 62.5 ps, which is the trapezoid phase error accumulated over the free ring (ngspice as-is has the same 5.9e-3 at tmax = h, so this is the method, not a RustSpice defect). Gates: checks/tran_order_and_conservation_oracles/check_conservation.py, checks/tran_order_and_conservation_oracles/check_order_convergence.py.
I1 0 n PULSE(0 10m 1n 1n 1n 1n 1)
L1 n 0 100n
C1 n 0 10p
.tran 62.5p 40n
.end
