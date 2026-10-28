Series RL step response, tau = L/R = 1 us
* bench.id: cap_el_rl_step_tran
* bench.group: capability
* bench.title: Series RL step response, tau = L/R = 1 us
* bench.probes: v(out), i(l1)
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: passive, tran, linear, inductor
* bench.note: Closed form: i(l1) = (1/100)*(1-exp(-t/1us)), v(out) = exp(-t/1us).
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
R1 in out 100
L1 out 0 100u
.tran 20n 5u
.end
