Buck converter
* bench.id: acc_tr_buck_converter_fine
* bench.group: accuracy
* bench.title: The same 12 V to ~5 V buck at tstep 10 ns (instead of 50 ns): RustSpice stops with a Newton failure at 61 us although ngspice solves it
* bench.probes: v(out)
* bench.tol.norm: 1.5e-2
* bench.expect: match
* bench.ref: reltol=1e-6 abstol=1e-12 vntol=1e-6
* bench.ref.tmax: 2.5n
* bench.tags: nonlinear, tran, mos, diode, power, switching, robustness
* bench.est_rss_mib: 16
* bench.note: companion of acc_tr_buck_converter (50 ns, passes). The ngspice reference is unchanged: reltol=1e-6 abstol=1e-12 vntol=1e-6 with tmax 2.5 ns (ngspice reports Timestep too small at abstol=1e-15 / vntol=1e-9). Failure times move with tstep (not tied to an edge: 1.25 us after turn-on at 12.5 ns), which is why the 50 ns deck passes by luck of the step grid. Probe v(sw) removed (v(out) kept, tolerances unchanged): v(sw) jumps 0 to 12 V within ~20 ns, so after resampling its max-norm is set by single samples (ngspice as-is vs this reference: 0.87 of the swing for v(sw) with RMS 0.024, against 4.8e-4 for v(out)) and even a correct trace could not pass tol.norm 1.5e-2 on it. Slug abstol-absolute-residual with linear-static-pivot-accuracy as co-cause (faer LU alone fixes only 5n and 10n); scaling the impedances (W and C divided, RL and L multiplied by 10 or 100, diode IS unchanged) does not remove the failure (it fails at 11.5 us and 24.0 us), so the stall is not a pure current-magnitude effect. With the faer LU (private patched build) the transient completes at the deck's 10 ns and v(out) agrees with the reference to 3.5e-4 of the swing.
VIN in 0 DC 12
M1 in g sw 0 NM W=500u L=1u
VG g sw PULSE(0 12 0 20n 20n 4u 10u)
D1 0 sw DM
L1 sw out 47u
C1 out 0 22u
RL out 0 5
.model NM NMOS(LEVEL=1 VTO=2 KP=20m LAMBDA=0.0)
.model DM D(IS=1e-10 N=1.0)
.tran 10n 100u
.end
