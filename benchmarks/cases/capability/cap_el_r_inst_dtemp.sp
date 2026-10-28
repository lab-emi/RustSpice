Resistor instance dtemp with tc1 and tc2
* bench.id: cap_el_r_inst_dtemp
* bench.group: capability
* bench.title: Resistor instance dtemp with tc1 and tc2
* bench.probes: i(v1)
* bench.expect: match
* bench.known_gap: silent-ignore-temperature
* bench.gap.note: dtemp, tc1 and tc2 are ignored: R stays 1000 ohm and i(v1)=-1 mA instead of -0.6557 mA
* bench.tags: passive, op, temperature, silent_risk
* bench.note: Exact: R = 1k*(1 + 0.01*50 + 1e-5*50^2) = 1525 ohm (tnom 27 C, instance temperature 27+50 C), i = -0.65574 mA. The .temp card is not used, so only the instance-level dtemp path is exercised.
V1 a 0 1
R1 a 0 1k tc1=0.01 tc2=1e-5 dtemp=50
.op
.end
