options reltol=-1 abstol=-1 vntol=-1 on a diode circuit
* bench.id: rob_hs_options_reltol_negative
* bench.group: robustness
* bench.title: .options reltol=-1 abstol=-1 vntol=-1 on a diode circuit
* bench.probes: v(2)
* bench.expect: either
* bench.tags: hostile, options, directive, silent_risk, ngspice_own_error
* bench.est_rss_mib: 64
* bench.note: AMBIGUOUS, no verdict (SUITE_SPEC 14.2 rule 2e). ngspice-47 cannot converge with negative tolerances (gmin/source stepping fail, 'The operating point could not be simulated successfully', rc 1). RustSpice ignores .options entirely (the registered silent-ignore-directive behaviour) and returns v(2)=0.6929 V, which is the true solution of the circuit (identical to ngspice with sane tolerances), because the solution does not depend on the tolerance settings. A rejection of reltol=-1 would be the ideal, but ngspice's failure is a convergence artefact rather than validation: nobody can be judged here
.options reltol=-1 abstol=-1 vntol=-1
V1 1 0 5
R1 1 2 1k
D1 2 0 DM
.model DM D(IS=1e-14)
.op
.end
