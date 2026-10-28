PWL with repeated time points (vertical step edges)
* bench.id: cap_el_v_pwl_step_edges
* bench.group: capability
* bench.title: PWL with repeated time points (vertical step edges)
* bench.probes: v(in)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.tags: source, tran, waveform, pwl, step, silent_risk
* bench.note: Equal consecutive time points are a vertical edge in ngspice-47 (it prints a warning about non-increasing PWL time points but runs and steps the output about 5 ps after the repeated time (measured at tmax 0.05n); away from the edges (1 ns) it equals the ideal 0/1/-1/0 staircase exactly). The edges (103.7n, 207.3n, 311.1n) are off the 5 ns sampling grid so no grid point sits on a jump. Levels: 0, 1, -1, 0. Accepting non-increasing PWL times (with a warning) is an ngspice leniency; the sampling grid avoids the edges, so the levels are unambiguous and RustSpice (constant 0 V, error 0.5 of swing) is wrong at every non-zero level [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 PWL(0 0 103.7n 0 103.7n 1 207.3n 1 207.3n -1 311.1n -1 311.1n 0 400n 0)
R1 in 0 1k
.tran 5n 400n
.end
