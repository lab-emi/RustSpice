DC source written dc=3.3
* bench.id: cap_el_v_dc_equals
* bench.group: capability
* bench.title: DC source written dc=3.3
* bench.probes: v(n1), i(v1)
* bench.expect: match
* bench.tags: source, op, parser, silent_risk
* bench.note: ngspice-47 accepts dc=3.3 as a DC value of 3.3 V (v(n1) = 3.3, tight reference). No waveform is present, so this is not a precedence defect: a valid token is read as something else without an error, and a precedence fix would not change the result (SUITE_SPEC 13.2 lists the dc=3.3 spelling under dc-value-ignored-with-waveform; the mechanism here is silent-misparse). Minimal flip: `dc 3.3` (cap_el_v_dc_forms) is read correctly. With a waveform `dc=2 SIN(1 1 1meg)` gives 1 V in RustSpice and 2 V in ngspice [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 n1 0 dc=3.3
R1 n1 0 1k
.op
.end
