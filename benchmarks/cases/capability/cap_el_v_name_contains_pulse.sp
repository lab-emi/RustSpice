Source whose NAME contains the letters of another waveform keyword
* bench.id: cap_el_v_name_contains_pulse
* bench.group: capability
* bench.title: Source whose name contains the letters of another waveform keyword
* bench.probes: v(in), v(in2)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.known_gap: silent-ignore-source-waveform
* bench.gap.note: the waveform keyword is searched as a substring of the whole card, so the name Vpulse1 makes RustSpice read the SIN(0 1 1meg) arguments as PULSE(0 1 1meg ...): v(in) stays at v1 = 0 V for the whole run (the 1 MHz frequency is read as a delay of 1e6 s) instead of a 1 V sine; renaming the source V1 flips it to correct (-1..+1 V); the Vsin2 PULSE source is the control: PULSE is searched before SIN, so it is read correctly
* bench.tags: source, tran, waveform, parser, sin, pulse, silent_risk
* bench.note: Both sources are written correctly. ngspice reads the source type from the card position, so Vpulse1 is a 1 MHz sine and Vsin2 is a pulse. Closed forms (ngspice matches them to 1e-15): v(in) = sin(2*pi*1e6*t); v(in2) = 3 V pulse, delay 0.5u, 100 ns edges, width 1u, period 3u. v(in2) is the control probe, it passes. A node named `pulse` (`V1 pulse 0 SIN(0 1 1meg)`) makes RustSpice read the source as 0 V in the same way (verified), while renaming Vpulse1 to V1 makes RustSpice agree with ngspice (1.5e-7 of swing)
Vpulse1 in 0 SIN(0 1 1meg)
R1 in 0 1k
Vsin2 in2 0 PULSE(0 3 0.5u 100n 100n 1u 3u)
R2 in2 0 1k
.tran 5n 4u
.end
