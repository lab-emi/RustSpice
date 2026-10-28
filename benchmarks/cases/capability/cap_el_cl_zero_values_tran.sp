Zero-valued capacitor and inductor in transient
* bench.id: cap_el_cl_zero_values_tran
* bench.group: capability
* bench.title: Zero-valued capacitor and inductor in transient
* bench.probes: v(o1), i(v2)
* bench.expect: match
* bench.tags: passive, tran, zero_value
* bench.note: C=0 is an open circuit (v(o1) = 0.5 V), L=0 is a short (i(v2) = -1 mA); v(o2) is identically 0 and not probed.
V1 in 0 PULSE(0 1 0 1u 1u 1 2)
R1 in o1 1k
R2 o1 0 1k
C1 o1 0 0
V2 in2 0 PULSE(0 1 0 1u 1u 1 2)
R3 in2 o2 1k
L1 o2 0 0
.tran 20u 1m
.end
