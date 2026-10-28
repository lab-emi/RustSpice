NMOS .model declared inside a .subckt
* bench.id: cap_dev_mos_model_in_subckt
* bench.group: capability
* bench.title: NMOS .model declared inside a .subckt
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, model_card, subckt, dc
* bench.note: evidence: RustSpice output is bit-identical with the .model removed or with a parameter-free .model; moving the same .model to top level matches tight ngspice to 1e-14 A. Closed form in saturation: 0.5*60u*(10/1)*(2-1.2)^2 = 1.92e-4 A (ngspice 1.92e-4 A); the default model (VTO=0, KP=2e-5) gives 0.5*2e-5*10*2^2 = 4.0e-4 A, which is what RustSpice prints [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
.subckt nx d g s b
M1 d g s b NMI W=10u L=1u
.model NMI NMOS(LEVEL=1 VTO=1.2 KP=60u)
.ends
VDS d 0 0
VG g 0 2
X1 d g 0 0 nx
.dc VDS 0 5 0.1
.end
