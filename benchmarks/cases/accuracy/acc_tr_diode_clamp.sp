Diode clamp (DC restorer)
* bench.id: acc_tr_diode_clamp
* bench.group: accuracy
* bench.title: Diode DC restorer: 0..5 V square wave through 1 uF onto a grounded-anode diode with 100 k load
* bench.probes: v(out)
* bench.tol.norm: 2e-2
* bench.tags: nonlinear, tran, diode, clamp, npoints
* bench.est_rss_mib: 16
* bench.note: A grid defect, not an accuracy loss: the converged ngspice reference sampled at RustSpice's own 402 row times and then resampled onto the 1 us grid the same way reproduces the identical 0.94 V (0.187), and RustSpice's values at its own rows differ from the reference by 4.7e-6 V (swing 5.01 V). The tag npoints puts the row count into the verdict (402 rows against 401 expected; ngspice as-is writes its own 460 rows). Flip tests (only tstep changed): 0.5 us and 0.25 us write the expected 801 and 1601 rows with 8.1e-10 and 2.0e-10 of swing error, 2 us desyncs again (202 rows, error 0.37).
V1 in 0 PULSE(0 5 10u 1u 1u 49u 100u)
C1 in out 1u
D1 0 out DMOD
R1 out 0 100k
.model DMOD D(IS=1e-14 N=1.0)
.tran 1u 400u
.end
