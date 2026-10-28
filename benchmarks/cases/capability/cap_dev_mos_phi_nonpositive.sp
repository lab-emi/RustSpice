NMOS .model with PHI=0 and GAMMA=0.5 (non-physical surface potential; ngspice rejects it)
* bench.id: cap_dev_mos_phi_nonpositive
* bench.group: capability
* bench.title: NMOS .model with PHI=0 and GAMMA=0.5 (non-physical surface potential; ngspice rejects it)
* bench.probes: i(vds)
* bench.expect: loud_error
* bench.tags: mosfet, model_card, op, ngspice_error_ok
* bench.note: ngspice stops with `Fatal error: nm: Phi is not positive` (exit 1), so refusing PHI=0 is correct. The mechanism is an evaluator floor, not a missing validation: RustSpice returns the identical i(vds) = -4.359067e-4 A for PHI=0, PHI=-0.5, PHI=0.001 and PHI=0.1 (both tools agree at PHI=0.1 and 0.11). A valid small PHI is therefore wrong too: PHI=1m gives -3.3257e-4 A in ngspice and in the closed form, RustSpice is +31 % off (cap_dev_mos_phi_small, expect match). Closed form with VTH = VTO + GAMMA*(sqrt(PHI-VBS) - sqrt(PHI)): PHI=0 gives VTH=1.2 V and 0.5*1e-3*0.8^2 = 3.2e-4 A; PHI=0.1 gives VTH=1.0663 V and 4.359e-4 A. With GAMMA=0 or VBS=0 PHI drops out and the tools agree. History: retagged from silent-ignore-model-param to lenient-malformed-input (section 13.2), then to param-floor-clamp after the floor was found
VDS d 0 3
VG g 0 2
VB b 0 -1
M1 d g 0 b NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u GAMMA=0.5 PHI=0)
.op
.end
