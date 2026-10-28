Include a file that defines a subcircuit
* bench.id: cap_lang_inc_subckt
* bench.group: capability
* bench.title: .include cap_lang_inc_subckt_div.lib supplies .subckt DIV (3k : 1k divider)
* bench.probes: v(out)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .include ignored: the observable outcome is LOUD, not silent (exit 1, "Unknown subcircuit `div`", misleading) because the subcircuit file was never read; ngspice gives v(out) = 2 V
* bench.tags: linear, op, include, subckt
* bench.note: the include path is relative to the deck's directory (ngspice-47 resolves it from the deck's directory whatever its own cwd, checked from a different cwd); ngspice computes v(out) = 8 V * 1k/(3k+1k) = 2 V; kept under silent-ignore-directive rather than unsupported-syntax-loud because the loud error is only a downstream symptom: .include itself is ignored without any message (cap_lang_inc_missing shows the same card accepted silently when the file does not exist)
V1 in 0 DC 8
X1 in out DIV
.include cap_lang_inc_subckt_div.lib
.op
.end
