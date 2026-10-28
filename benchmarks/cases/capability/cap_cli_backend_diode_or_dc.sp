Two-diode OR selector with a swept input: DC sweep for the backend matrix
* bench.id: cap_cli_backend_diode_or_dc
* bench.group: capability
* bench.title: Two-diode OR selector (V1 swept 0..4 V against a fixed 2.5 V input) with different diode models, DC-sweep backend-matrix fixture
* bench.probes: v(out), v(p), i(v1), i(v2)
* bench.expect: match
* bench.tol.rel: 1e-8
* bench.tol.abs: 1e-10
* bench.est_rss_mib: 40
* bench.tags: diode, dc, sweep, nonlinear, kink, backend_fixture, cli_surface
* bench.note: fixture of checks/untested_cli_surface/check_backend_equiv.py (DC-sweep path of every backend); the selector hands over from V2 to V1 near 2.5 V so the sweep crosses a smooth but sharp conduction exchange. The deck itself is an ordinary DC-sweep comparison against ngspice.
V1 in1 0 DC 0
V2 in2 0 DC 2.5
R1 in1 p 100
R2 in2 q 100
D1 p out DM1
D2 q out DM2
R3 out 0 2k
.model DM1 D(IS=1e-14 N=1)
.model DM2 D(IS=4e-15 N=1.1)
.dc V1 0 4 0.1
.end
