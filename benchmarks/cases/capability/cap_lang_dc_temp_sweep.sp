DC sweep of temperature
* bench.id: cap_lang_dc_temp_sweep
* bench.group: capability
* bench.title: .dc temp -40 120 40 sweeps the circuit temperature
* bench.probes: v(d)
* bench.tags: nonlinear, dc, temperature
* bench.note: tight ngspice-47 gives v(d) = 0.80071, 0.73682, 0.67152, 0.60500, 0.53742 V at -40, 0, 40, 80, 120 C; the textbook IS(T) = IS*exp((T/Tnom-1)*EG/(N*Vt))*(T/Tnom)^(XTI/N) (EG 1.11, XTI 3, Tnom 27 C, gmin 1e-12) solved in mpmath agrees within 3e-7 V at every point, so the reference is sound Parsing `.dc temp` alone would not close the gap: RustSpice ignores temperature everywhere (silent-ignore-temperature), so gap_closed also needs the diode temperature physics. Reference accuracy: the tight ngspice run is within 3e-7 V (5.5e-7 relative at worst) of the mpmath solve at all five points; the as-is ngspice run is within 3.9e-5 V (5.8e-5 relative), which is why the tight reference is used. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 5
R1 in d 1k
D1 d 0 DM
.model DM D(IS=1e-14)
.dc temp -40 120 40
.end
