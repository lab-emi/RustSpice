latin-1 title ต้
* bench.id: rob_hs_ff_latin1_comment
* bench.group: robustness
* bench.title: Latin-1 (invalid UTF-8) bytes in the title and a comment line
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: RustSpice reads the deck as a UTF-8 string and refuses the whole file ('stream did not contain valid UTF-8') because of Latin-1 bytes in the title and one comment line; ngspice-47 reads it (v(2)=2.5 V)
* bench.tags: file, encoding
* bench.est_rss_mib: 64
* bench.note: legacy decks carry Latin-1 characters in titles and comments; the bytes are only in places that carry no circuit semantics, so a correct reader ignores them. Contrast rob_hs_ff_invalid_utf8_in_node, where the invalid byte is inside a node name and ngspice refuses too Same gap as cap_lang_syn_invalid_utf8_comment (the whole-file UTF-8 read); here the Latin-1 bytes sit in the title and in a comment line.
* comment with latin-1 bytes ต ้ ่ ÿ
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
