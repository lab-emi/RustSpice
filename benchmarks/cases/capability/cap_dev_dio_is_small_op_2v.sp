Diode IS=1e-16 from 2 V through 100 ohm: OP
* bench.id: cap_dev_dio_is_small_op_2v
* bench.group: capability
* bench.title: Diode IS=1e-16 from 2 V through 100 ohm: OP
* bench.probes: v(a), i(v1)
* bench.expect: match
* bench.tags: diode, op, small_is, silent_risk
* bench.note: new deck: second witness of the cap_dev_dio_is_small_op defect at IS=1e-16 (a normal small-signal value). Hand-solved Shockley (mpmath, kT/q at 300.15 K, gmin 1e-12) v(a)=0.837677 V, i=11.623 mA; tight ngspice 0.837677 V (3e-7 V apart with the CODATA kT/q; with kT/q = 0.0258649170071575 V (the constant behind ngspice-47) mpmath gives 0.8376770281 V, equal to tight ngspice to 1e-16 relative). Python scan of 135 (IS,V,R) combinations vs mpmath: RustSpice `.op` is wrong in 56, all with IS<=1e-16 (IS=1e-16: only 1 V/100 ohm and 2 V/100 ohm; 1e-17: 7; 1e-18: 11; 1e-20 and below: 12 each) and in none of the 45 with IS>=1e-15, so it depends on the Newton path (V, R, IS), not on IS alone; the same circuit as a `.dc` sweep is right
V1 in 0 2
R1 in a 100
D1 a 0 DM
.model DM D(IS=1e-16)
.op
.end
