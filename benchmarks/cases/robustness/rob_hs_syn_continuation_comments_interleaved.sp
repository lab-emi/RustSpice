continuation lines mixed with comments, blanks and indentation
* bench.id: rob_hs_syn_continuation_comments_interleaved
* bench.group: robustness
* bench.title: continuation lines interleaved with comment lines, blanks and indentation
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: syntax, parser, continuation, lenient_ok
* bench.est_rss_mib: 64
* bench.note: the subcircuit header and the cards R2 and R9 are spread over continuation lines interleaved with full-line comments (also indented ones), inline comments and whitespace-only continuations; the deck ends with .end plus trailing blanks and a duplicated .end. Expected: R1 (1k) in series with R2 || R9 (500 ohm): v(2)=1.667 V, v(1)=5 V, i(v1)=-3.333 mA; a lost continuation changes the numbers or is refused
.subckt tri a
+ b
   * indented comment between header lines
+ c ; inline comment
R1 a b 1k
R2 b c
+ 1k
.ends
V1 1 0 5
X1 1 2 0 tri
R9 2
* comment between a card and its continuation
   + 0
	+ ; nothing but a comment
+ 1k
.op
.end   	 
.END
