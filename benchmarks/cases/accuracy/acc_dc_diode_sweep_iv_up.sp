Diode I-V sweep -5 V .. +1 V through 1k, increasing
* bench.id: acc_dc_diode_sweep_iv_up
* bench.group: accuracy
* bench.title: Diode I-V sweep -5 V .. +1 V through 1k, increasing
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-17
* bench.floor: 1e-14
* bench.tags: nonlinear, dc, diode, sweep
* bench.note: reverse leakage 5e-12 A at the reverse end (tol.abs 1e-17 A); no dc_nonlinear oracle (reverse region follows the SPICE3 reverse law)
V1 in 0 0
R1 in d 1k
D1 d 0 dm
.model dm D(IS=1e-14 N=1)
.dc V1 -5 1 0.05
.end
