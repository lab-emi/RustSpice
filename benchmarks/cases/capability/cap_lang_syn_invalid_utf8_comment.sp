Invalid UTF-8 byte in a comment
* bench.id: cap_lang_syn_invalid_utf8_comment
* bench.group: capability
* bench.title: a Latin-1 byte (0xE9) and a stray 0xFF in comment lines make the file invalid UTF-8
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud error "stream did not contain valid UTF-8" (exit 1) for a Latin-1 byte (0xE9) and a stray 0xFF inside a comment line: the whole file is read as one UTF-8 string
* bench.tags: linear, op, syntax, encoding
* bench.note: ngspice-47 ignores the invalid bytes in the comment lines and computes v(out) = 8 V * 1k/(3k+1k) = 2 V (hand-checked) Duplicate of rob_hs_ff_latin1_comment (one gap: the whole-file UTF-8 read). Verified flips: the same comment as valid UTF-8 or as ASCII gives v(out) = 2 V in RustSpice; a Latin-1 byte in the title line or after a `$` is refused as well.
* résistance ÿ
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.op
.end
