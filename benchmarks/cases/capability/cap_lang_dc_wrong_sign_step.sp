DC sweep with a step of the wrong sign
* bench.id: cap_lang_dc_wrong_sign_step
* bench.group: capability
* bench.title: .dc V1 5 0 1 (start > stop with a positive step) is an empty or invalid sweep
* bench.probes: v(out)
* bench.expect: either
* bench.tags: linear, dc, invalid_input, ngspice_empty
* bench.note: genuinely ambiguous, no verdict is possible: ngspice-47 treats start > stop with a positive step as an empty sweep (exit 0, header-only rawfile with 0 data rows, no message), RustSpice silently sweeps 5 -> 0 (6 points, the same numbers as the explicit '.dc V1 5 0 -1' deck cap_lang_dc_negative_step); the intent of a downward sweep is unambiguous but nothing says whether an empty result, an error or a sign-agnostic sweep is correct, and an empty rawfile gives the suite no reference numbers
V1 in 0 0
R1 in out 1k
R2 out 0 3k
.dc V1 5 0 1
.end
