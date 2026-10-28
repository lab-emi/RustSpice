NMOS cutoff 10 mV below threshold with a 1 Mohm load: drain stays at VDD
* bench.id: cap_dev_mos_cutoff_load
* bench.group: capability
* bench.title: NMOS cutoff 10 mV below threshold with a 1 Mohm load: drain stays at VDD
* bench.probes: v(d)
* bench.tol.abs: 3e-8
* bench.tol.rel: 1e-9
* bench.expect: match
* bench.tags: mosfet, level1, op, cutoff
* bench.note: sharpened (was RD=1k, VG=0.3, default tolerances = 3e-6 V, blind to leakage): RD=1 Mohm turns leakage into volts, v(d)=3-RD*leak=2.999997 V; tol 3e-8 V resolves 3e-14 A of spurious conduction (RustSpice at VG=0.7001 already drops v(d) by 5e-6 V). The accepted 1.0e-8 V RustSpice-vs-ngspice offset is ngspice's 1e-14 A bulk-junction saturation current times 1 Mohm (RustSpice has no bulk junctions, see cap_dev_mos_bulk_diode)
VDD vdd 0 3
RD vdd d 1Meg
VG g 0 0.69
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u)
.op
.end
