1 uA current source charging a 1 nF capacitor from rest (DC-singular initial OP), 1 us transient
* bench.id: rob_cv_sing_isrc_into_cap_tran
* bench.group: robustness
* bench.title: 1 uA current source charging a 1 nF capacitor from rest (DC-singular initial OP), 1 us transient
* bench.probes: v(1)
* bench.expect: either
* bench.tags: ill_posed, current_cutset, silent_risk, ngspice_error_ok, tran
* bench.note: Without .ic/uic the initial DC operating point does not exist (0*v(1) = 1 uA), so the transient has no defined start. Verified with both tools: RustSpice exits 0 silently and writes a flat 1e6 V trace (the exact solution of the capacitor in parallel with the gmin regularisation, slope 0); ngspice-47 warns "singular matrix: check node 1" at the OP, starts at 0.01 V and ramps +1000 V/s (v(1 us) = 0.011 V). The exact ramp from rest is v(1) = I*t/C = 1 mV at 1 us; both tools differ from it (RustSpice by 1e6 V, ngspice by a 10 mV offset), so neither can be judged and expect: either. With a defined initial state both tools agree: see rob_cv_sing_isrc_into_cap_tran_ic0.
I1 0 1 1u
C1 1 0 1n
.tran 10n 1u
.end
