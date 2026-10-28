Quoted file name and the .inc spelling
* bench.id: cap_lang_inc_quoted_short
* bench.group: capability
* bench.title: .inc "cap_lang_inc_file_models.lib" uses the short spelling and a double-quoted path
* bench.probes: v(out)
* bench.tags: nonlinear, op, include, syntax
* bench.note: reuses the include target of cap_lang_inc_file; the path is relative to the deck's directory
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DINC
.inc "cap_lang_inc_file_models.lib"
.op
.end
