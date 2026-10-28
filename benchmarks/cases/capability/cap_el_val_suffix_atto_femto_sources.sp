Scale suffixes a f p n u on source values (each source forces 1 mA into 1k)
* bench.id: cap_el_val_suffix_atto_femto_sources
* bench.group: capability
* bench.title: Scale suffixes a f p n u on source values (each source forces 1 mA into 1k)
* bench.probes: v(a), v(b), v(c), v(d), v(e), v(f), v(g)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-15
* bench.floor: 1e-6
* bench.expect: match
* bench.tags: parser, source, op, value_syntax
* bench.note: Resistors are not used to test the small suffixes (ngspice clamps R below 1e-12 ohm). Instead each current source value is 1 mA written with a different suffix, 1e15a, 1e12f, 1e9p, 1e6n, 1e3u and 1e3U, and a G with the transconductance 1e15a is driven by 1 V so the same suffix is read on a G card: v(a)..v(f) = 1 V, v(g) = 1e-3*1 V*1k = 1 V. ngspice reads a as atto (1e-18) and RustSpice does as well.
I1 0 a 1e15a
R1 a 0 1k
I2 0 b 1e12f
R2 b 0 1k
I3 0 c 1e9p
R3 c 0 1k
I4 0 d 1e6n
R4 d 0 1k
I5 0 e 1e3u
R5 e 0 1k
I6 0 f 1e3U
R6 f 0 1k
V1 in 0 1
G1 0 g in 0 1e15a
R7 g 0 1k
.op
.end
