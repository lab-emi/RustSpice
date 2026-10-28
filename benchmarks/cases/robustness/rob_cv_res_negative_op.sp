Negative resistor: 1k feeding a -3k load to ground (exact v(2)=7.5 V)
* bench.id: rob_cv_res_negative_op
* bench.group: robustness
* bench.title: Negative resistor: 1k feeding a -3k load to ground (exact v(2)=7.5 V)
* bench.probes: v(2), i(v1)
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: linear, negative_resistance, silent_risk, op
* bench.note: ngspice-47 gives the algebraically exact 7.5 V. Even a loud refusal would be acceptable for R<0; silent substitution is not. Calibration evidence: closed form v(2)=5*(-3k)/(1k-3k)=7.5 V and i(v1)=+2.5 mA are reproduced exactly by ngspice; the same deck with R2=+3k gives the correct 3.75 V in RustSpice, so only the sign handling differs. Oracle: the declared dc_linear oracle rejects R<=0 ("r2: value must be > 0", status unsupported), so the closed form above is the independent oracle. RustSpice output is bit-identical for R2 = -3k, -1, -1m, -1e-6, 0 and +1e-18 (sign and magnitude are lost), while ngspice honours the sign (R2 = -1m: v(2) = -5.000005 uV, i(v1) = -5.000005 mA).
V1 1 0 5
R1 1 2 1k
R2 2 0 -3k
.op
.end
