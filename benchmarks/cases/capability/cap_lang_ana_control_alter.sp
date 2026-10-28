Control block that changes the circuit before the analysis
* bench.id: cap_lang_ana_control_alter
* bench.group: capability
* bench.title: a .control block that alters a source (alter V1 = 9) before run changes the answer
* bench.probes: v(out)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-analysis
* bench.gap.note: .control block ignored: .op prints v(out)=4 V where ngspice (alter V1=9) gives 6 V
* bench.tags: linear, control, unsupported_analysis
* bench.note: ngspice-47 executes the .control block (alter V1 = 9, run): v(out) = 9 V * 2k/3k = 6 V (hand-checked); .control is ngspice's scripting language, outside a netlist tool's model, so refusing is the correct behaviour and silently skipping it (4 V, no message) is the defect
V1 in 0 DC 6
R1 in out 1k
R2 out 0 2k
.op
.control
alter V1 = 9
run
.endc
.end
