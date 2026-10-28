Same OTA follower, input swept 0.9..2.4 V in 50 mV steps (output tracks within 0.1 mV)
* bench.id: rob_cv_ota_follower_dc
* bench.group: robustness
* bench.title: Same OTA follower, input swept 0.9..2.4 V in 50 mV steps (output tracks within 0.1 mV)
* bench.probes: v(out)
* bench.tol.abs: 1e-8
* bench.expect: match
* bench.tags: nonlinear, mos, feedback, high_gain, dc
* bench.note: ngspice-47 needs dynamic gmin stepping (its log shows Starting/completed dynamic gmin stepping for this .dc deck only; the .op sibling is solved without stepping) and completes the sweep (output error vs input < 0.13 mV). Reference verified: every ngspice-tight sweep point is an exact root of an independent dense f64 Level-1 KCL system (node deviation 2.5e-8 V from the gmin placement, residual 1e-19 A) and random-start Newton at 0.9, 1.5 and 2.4 V finds no other root. Retagged from clamp-rail: the solution is inside the rails and the failure persists for a single sweep point; same Newton mechanism as rob_cv_ota_follower_op. Flips: forcing the faer LU or abstol=1e-9 does not help (private patched build, reproduced). Second-look flip (private patched build, not reproduced here): a 0.3 V global node-step limit makes all 31 points converge and match ngspice, a 1 V limit does not; fetlim is defined in the CPU backend but never called. Slug kept at the registered dc-static-homotopy (dc-newton-no-limiting is not registered; the closest registered slug).
Vdd vdd 0 3.3
Vin inp 0 1.5
It tail 0 20u
M1 d1 out tail 0 nm w=20u l=1u
M2 d2 inp tail 0 nm w=20u l=1u
M3 d1 d1 vdd vdd pm w=20u l=1u
M4 d2 d1 vdd vdd pm w=20u l=1u
M5 out d2 vdd vdd pm w=40u l=1u
Ib out 0 40u
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.7 kp=40u lambda=0.02)
.dc Vin 0.9 2.4 0.05
.end
