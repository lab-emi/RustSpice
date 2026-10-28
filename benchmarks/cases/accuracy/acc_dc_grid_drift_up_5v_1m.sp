DC sweep 0 .. 5 V in 1 mV steps: 5001 points, last point at 5.000 V
* bench.id: acc_dc_grid_drift_up_5v_1m
* bench.group: accuracy
* bench.title: DC sweep 0 .. 5 V in 1 mV steps through a 3:1 divider: all 5001 points, including the 5.000 V end point, must be produced
* bench.probes: v(out), i(v1)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.size: 5001
* bench.tags: linear, dc, sweep, grid, many_points
* bench.note: closed form v(out) = V1/3 (1.6666667 V at the 5 V end point), i(v1) = -V1/3000; the other 5000 points agree with ngspice to 1e-16 and their sweep values are bit-identical in the two tools (RustSpice's last written point is 4.999000000000004 V, ngspice's 5001st is 5.000000000000004 V; measured). Survey (RustSpice point count vs expected): 0..5 V at 1 mV and 0.1 mV, 5..0 V at -1 mV, -5..5 V at 1 mV, 1..6 V at 1 mV, 0..100 V at 10 mV, 0..7 V at 1 mV and 0..1 V at 1 uV drop the last point; 0..1, 0..2, 0..3, 0..4 and 0..10 V at 1 mV, 0..3.3 V at 10 mV and 0..5 V at 2 mV/5 mV/10 mV do not (acc_dc_grid_many_points_1001). The count mismatch fails the case as a whole (the engine does not compare values then); ngspice itself drifts by 3e-13 V at the last point of this deck but keeps it. That holds for the two decks of this slug only: of the 8 sweeps above where RustSpice drops the last point, ngspice (measured) also drops it for 0..5 V at 0.1 mV, 1..6 V at 1 mV, 0..100 V at 10 mV, 0..7 V at 1 mV and 0..1 V at 1 uV, and keeps it for 0..5 V at 1 mV, 5..0 V at -1 mV and -5..5 V at 1 mV; the two decks of this slug are sweeps on which ngspice and the integer count floor(|stop-start|/|step| + 1e-9) + 1 agree (5001 points).
V1 in 0 0
R1 in out 2k
R2 out 0 1k
.dc V1 0 5 1m
.end
