form feed and vertical tab used as token separators
* bench.id: rob_hs_ff_formfeed_vtab_separators
* bench.group: robustness
* bench.title: form feed and vertical tab characters separate the tokens of a card
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: file, parser, lenient_ok
* bench.est_rss_mib: 64
* bench.note: the V1 and R2 cards use 0x0C (form feed) and 0x0B (vertical tab) between the tokens; ngspice-47 treats them as blanks (v(2)=2.5 V, i(v1)=-2.5 mA) and so must RustSpice, otherwise the card is refused or misread
V1105
R1 1 2 1k
R2201k
.op
.end
