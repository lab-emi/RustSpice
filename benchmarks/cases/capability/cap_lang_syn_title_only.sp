A deck with only a title
* bench.id: cap_lang_syn_title_only
* bench.group: capability
* bench.title: title line and metadata but no circuit element and no analysis card
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: syntax, empty, invalid_input, ngspice_error_ok
* bench.note: ngspice-47 exits 0 but writes no rawfile (nothing to simulate); RustSpice refuses ('Circuit contains no non-ground nodes or branches'); a circuit without elements has no solution to report, so refusing is the correct behaviour and silently printing an empty result would be the defect
