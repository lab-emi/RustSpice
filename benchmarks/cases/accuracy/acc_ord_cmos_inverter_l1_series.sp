CMOS inverter driven by a smooth sine, MOS level 1
* bench.id: acc_ord_cmos_inverter_l1_series
* bench.group: accuracy
* bench.title: CMOS inverter (level 1, 3.3 V, 200 fF) driven by SIN(1.65 1.65 500meg) through the switching region, 2 periods, order series
* bench.probes: v(out), i(vdd)
* bench.tol.norm: 1.5e-3
* bench.expect: match
* bench.series.tstep: 40p 20p 10p 5p 2.5p
* bench.tags: nonlinear, tran, mos, cmos, sin, series, order
* bench.est_rss_mib: 16
* bench.note: First MOS transient with an independent oracle (acc_tr_cmos_inv_step and acc_tr_cmos_chain5 are judged against ngspice only). Truth is the level-1 Shichman-Hodges equations written out by hand (lambda applied in triode and saturation, GMIN 1e-12 S across each channel), C dv/dt = idp - idn, scipy Radau rtol 1e-11 cross-checked with DOP853 to 2.5e-11 of the swing; the same equations reproduce the DC output of both tools at 8 input voltages to 3e-12 V (ngspice) and 1.2e-9 V (RustSpice, at the high-gain point), and i(vdd) = -idp matches RustSpice to 9.4e-7 A. No intrinsic gate or junction capacitance (no TOX/CGSO/CJ), only the explicit 200 fF. RustSpice error vs truth 4.83e-2/1.22e-2/3.05e-3/7.62e-4/1.91e-4 V for h = 40p..2.5p (pair slopes 2.00); the suite reference run is within 1.35e-6 V at the nominal 10p; ngspice as-is is 5.3x more accurate than RustSpice at equal tmax = h (5.7e-4 V at 10p, also order 2). tol.norm 1.5e-3 is 1.6x RustSpice's own truncation error at 10p; coarser members are info. Gate: checks/tran_order_and_conservation_oracles/check_order_convergence.py.
VDD vdd 0 DC 3.3
VIN in 0 SIN(1.65 1.65 500meg)
MP1 out in vdd vdd PMOD W=20u L=1u
MN1 out in 0 0 NMOD W=10u L=1u
CL out 0 200f
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 10p 4n
.end
