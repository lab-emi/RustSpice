Current source polarity and forms (current flows n+ to n- through the source)
* bench.id: cap_el_i_dc_forms_polarity
* bench.group: capability
* bench.title: Current source polarity and forms (current flows n+ to n- through the source)
* bench.probes: v(a), v(b), v(c), v(d), v(e)
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: source, op, polarity
* bench.note: I 0 x pushes current into x: v(a)=+1, v(b)=-1, v(c)=v(d)=+1, v(e)=+5 V.
I1 0 a 1m
R1 a 0 1k
I2 b 0 1m
R2 b 0 1k
I3 0 c dc 1m
R3 c 0 1k
I4 0 d dc 1m ac 1
R4 d 0 1k
I5 0 e 2.5m
R5 e 0 2k
.op
.end
