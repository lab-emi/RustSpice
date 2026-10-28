1 uA current source charging a 1 nF capacitor from a defined initial state (.ic v(1)=0, uic), 1 us transient
* bench.id: rob_cv_sing_isrc_into_cap_tran_ic0
* bench.group: robustness
* bench.title: 1 uA current source charging a 1 nF capacitor from a defined initial state (.ic v(1)=0, uic), 1 us transient
* bench.probes: v(1)
* bench.expect: match
* bench.tags: ill_posed, current_cutset, ic, uic, tran
* bench.note: Control for rob_cv_sing_isrc_into_cap_tran: the DC operating point of this circuit does not exist (0*v(1) = 1 uA), but .ic v(1)=0 with uic defines the initial state, and then the answer is unique and closed-form: v(1) = I*t/C = 1000 V/s * t, i.e. 1 mV at 1 us (independent of any simulator; the tran_lti oracle is not declared because it does not support uic). ngspice-47 skips the OP and starts the ramp at 0, RustSpice applies the .ic after its OP and also writes the exact ramp (its output does not depend on the uic keyword); RustSpice is within 5e-13 V of the closed form at every one of its 101 rows (5e-10 of the 1 mV swing, checked in Python) and within 5e-10 of the swing of the converged ngspice reference, ngspice as-is within 5e-5; the default tol.norm 5e-3 is kept and the probe is not vacuous (scaling C by 5 % moves the ramp by 5 % of the swing).
I1 0 1 1u
C1 1 0 1n
.ic v(1)=0
.tran 10n 1u uic
.end
