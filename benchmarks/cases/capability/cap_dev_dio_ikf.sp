Diode high-injection knee IKF=1 mA
* bench.id: cap_dev_dio_ikf
* bench.group: capability
* bench.title: Diode high-injection knee IKF=1 mA
* bench.probes: v(a), i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, model_param
* bench.note: Gap closed 2026-10-28: RustSpice now models high-injection knee IKF; the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without IKF; Id=Idl/(1+sqrt(Idl/IKF)) solved with the 10 ohm series resistor equals tight ngspice to 7e-6 relative; RustSpice current is 11% too high at 2 V (122.1 vs 110.1 mA, v(a) 0.119 V off) and 89% too high at 0.7 V; the knee law is dialect-defined (black-box fit of ngspice): the pass criterion is ngspice's law, the alternative Idl*sqrt(IKF/(IKF+Idl)) differs from it by up to 41% in current at the knee and 3.6 mV in v(a), still 33x less than the RustSpice error
V1 in 0 0
R1 in a 10
D1 a 0 DM
.model DM D(IS=1e-14 IKF=1e-3)
.dc V1 0 2 0.1
.end
