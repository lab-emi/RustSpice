Half-wave rectifier at 10 MHz into 100 pF parallel 2 k
* bench.id: acc_ord_diode_halfwave_rc_series
* bench.group: accuracy
* bench.title: Half-wave rectifier, SIN(0 3 10meg) through 100 ohm into 100 pF || 2 k (IS = 1e-14, N = 1), 20 periods, order series
* bench.probes: v(out), v(a)
* bench.tol.norm: 3e-4
* bench.expect: match
* bench.series.tstep: 2n 1n 500p 250p 125p
* bench.tags: nonlinear, tran, diode, rectifier, sin, series, order
* bench.est_rss_mib: 16
* bench.note: Nonlinear order deck with an independent oracle; the existing rectifier decks (acc_tr_diode_halfwave, _halfwave_rail, _peak_detector) have none and run at 1-10 kHz (on acc_tr_diode_halfwave_rail RustSpice's error vs the truth is non-monotone: 2.1e-2/8.6e-3/8.7e-3/2.1e-3 V for h = 10u..1u, see checks/tran_order_and_conservation_oracles/check_existing_series_reference.py). Truth is the scalar ODE C dv/dt = iD(vd) - v/R2 with the anode node eliminated exactly by Lambert W, scipy Radau rtol 1e-11 cross-checked with DOP853 to 8e-12 of the swing; Vt = 25.8649170072 mV and GMIN = 1e-12 S were calibrated on the DC operating point of both tools (residual 1e-16 V ngspice, 4e-11 V RustSpice). RustSpice error vs truth 5.35e-3/1.11e-3/2.74e-4/6.85e-5/1.71e-5 V for h = 2n..125p (pair slopes 2.02/2.00/2.00 for h <= 1n); the suite reference run is within 6.8e-7 V (3.5e-7 of swing) at the nominal 500p; ngspice with reltol=1e-9 and tmax=h floors at 5.1e-6..5.4e-6 V. tol.norm 3e-4 is 2.1x RustSpice's own truncation error at 500p; coarser members are info. Gate: checks/tran_order_and_conservation_oracles/check_order_convergence.py.
V1 in 0 SIN(0 3 10meg)
R1 in a 100
D1 a out DMOD
C1 out 0 100p
R2 out 0 2k
.model DMOD D(IS=1e-14 N=1.0)
.tran 500p 200n
.end
