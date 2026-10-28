NPN Gummel-Poon with VAF/IKF/ISE/RB/RE (collector family)
* bench.id: cap_dev_bjt_gummel_poon
* bench.group: capability
* bench.title: NPN Gummel-Poon with VAF/IKF/ISE/RB/RE (collector family)
* bench.probes: i(vce)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: Q card refused loudly (exit 1, ParseError `Unsupported device card Q1`); ngspice solves the 21-point family (|i(vce)| up to 1.56 mA)
* bench.tags: bjt, dc, unsupported
* bench.note: reference verified independently: own Gummel-Poon solve (python fsolve: VAF early effect, IKF high-injection qb, ISE/NE base recombination, RB/RE/RC series resistances, gmin) equals tight ngspice to 2.7e-8 relative on every sweep point
VCE c 0 0
IB 0 b 20u
Q1 c b 0 QN
.model QN NPN(IS=1e-15 BF=100 VAF=50 IKF=5m ISE=1e-14 NE=1.5 RB=100 RE=1 RC=5)
.dc VCE 0 5 0.25
.end
