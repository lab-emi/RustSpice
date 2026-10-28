Diode IS=1e-14 with series resistance RS=10 ohm from 5 V through 100 ohm
* bench.id: acc_dc_diode_fwd_series_rs
* bench.group: accuracy
* bench.title: Diode IS=1e-14 with series resistance RS=10 ohm from 5 V through 100 ohm
* bench.probes: v(d), i(v1)
* bench.expect: match
* bench.tags: nonlinear, op, diode, model_param, silent_risk
* bench.note: Gap closed 2026-10-28: RustSpice now models diode series resistance RS (internal node); the evidence below records the reference and the behaviour before the fix. closed form: v(d) = 5 - 100 i with i from IS exp((vj)/vt), vj = v(d) - 10 i; independent f64 root-find (brentq, gmin 1e-12, CODATA constants at 300.15 K) gives v(d) = 1.136032535 V, i = 38.63967 mA vs tight ngspice 1.136032305 V (2.3e-7 V, the physical-constant set); an explicit 10 ohm resistor in RustSpice reproduces it, so only the parameter is lost
V1 in 0 5
R1 in d 100
D1 d 0 dm
.model dm D(IS=1e-14 N=1 RS=10)
.op
.end
