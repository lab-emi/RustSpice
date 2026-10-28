Include of a file that does not exist
* bench.id: cap_lang_inc_missing
* bench.group: capability
* bench.title: .include cap_lang_inc_missing_nonexistent.lib names a file that is not there
* bench.probes: v(out)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-directive
* bench.gap.note: missing include file is not noticed; the .op runs (exit 0, empty stderr, v(out) = 2 V); ngspice-47 aborts ('Could not find include file', exit 1)
* bench.tags: linear, op, include, invalid_input, ngspice_error_ok
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.include cap_lang_inc_missing_nonexistent.lib
.op
.end
