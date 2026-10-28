NMOS L1 substrate doping NSUB=1e16 (derives PHI/GAMMA/VTO) with body bias
* bench.id: cap_dev_mos_nsub
* bench.group: capability
* bench.title: NMOS L1 substrate doping NSUB=1e16 (derives PHI/GAMMA/VTO) with body bias
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: NSUB/TOX ignored: no derived PHI/GAMMA/VTO, no body effect (2.00 mA at Vb=-3 V, ngspice 1.78 mA)
* bench.tags: mosfet, level1, dc
* bench.note: evidence: RustSpice output is bit-identical with and without NSUB/TOX (KP is given, so TOX only enters through the derived GAMMA/PHI/VTO); tight ngspice moves by 2.2e-4 A of a 3.6e-4 A swing (ngspice default vs tight differ by 4e-7 A). Independent oracle: the textbook derivation (PHI = 2*Vt*ln(NSUB/ni), GAMMA = sqrt(2*q*eps_si*NSUB)/Cox, VTO = -(Eg/2 + phiF) + PHI + GAMMA*sqrt(PHI) with Eg = 1.16 - 7.02e-4*T^2/(T+1108), eps_si = 11.7*eps0, ni = 1.45e10 /cm3) gives VTO/GAMMA/PHI = -0.07068/0.16685/0.69545 and reproduces tight ngspice's id(VB) to 6.6e-7 relative over the 13 points (a fit of VTO/GAMMA/PHI to the ngspice sweep gives -0.070676/0.166849/0.695453, residual 1.7e-10, and RustSpice with those explicit parameters matches ngspice to 5.6e-12 A). Closure needs ni = 1.45e10: ni = 1.5e10 gives 1.0e-3 relative error, above tol.rel
VDS d 0 3
VG g 0 2
VB b 0 0
M1 d g 0 b NM W=10u L=1u
.model NM NMOS(LEVEL=1 KP=100u NSUB=1e16 TOX=1e-8)
.dc VB -3 0 0.25
.end
