Diode transit time TT=50 ns (diffusion capacitance): 2 MHz sine around 0.65 V bias
* bench.id: cap_dev_dio_tt
* bench.group: capability
* bench.title: Diode transit time TT=50 ns (diffusion capacitance): 2 MHz sine around 0.65 V bias
* bench.probes: i(v1)
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: TT ignored: no diffusion capacitance, current lead absent: norm err 0.33
* bench.tags: diode, tran, transit_time, model_param
* bench.note: evidence: RustSpice output is bit-identical with and without TT; closed form (V1 ideal across the diode: i=-(Id+TT*gd*dv/dt)) equals the converged ngspice reference to 3.3e-5 normalised for t>0 (the t=0 row is the DC OP in both tools) and RustSpice equals the TT-free closed form to 8e-6
V1 a 0 SIN(0.65 0.03 2Meg)
D1 a 0 DM
.model DM D(IS=1e-14 TT=50n)
.tran 5n 2u
.end
