#!/usr/bin/env python3
"""Reference-free relations of the transient engine on linear RC ladders.

(i)  time-scale invariance: scale EVERY capacitor, every source time (td, tr, tf, pw, per, 1/freq) and tstep/tstop by
     alpha in {1e-3, 1e3, 1e6}; R and the voltages are unchanged, so every node voltage at row k must be identical
     (the fixed-step companion matrix C/h and the source samples v(k*h/alpha) are unchanged).
(ii) time-shift invariance: delaying the sources by exactly m*tstep must delay every node waveform by exactly m rows
     (the circuit is at rest during the first (BDF-1) step in both runs).

RustSpice only is gated (it has a fixed step grid, so both relations hold to rounding).  ngspice (adaptive steps) is a
black-box control: its deviation under the same transformation is printed, it is NOT gated.
Gate: max |v_alpha - v_1| <= 1e-11 V on waveforms of >= 0.05 V swing.  The residual is not zero because the time loop
accumulates t += h (k rounding errors of 1.1e-16 relative on t_k, i.e. <= 500 * 1.1e-16 * 5 ns = 2.8e-22 s) and the pulse
edge slope is 1e10 V/s, a worst case of 2.8e-12 V; observed ~2e-13.  Exit code 1 on violation.
--selftest injects a 1e-6 relative error into the scaled capacitors and exits 0 only if the gate rejects it.
Rule 9: two ladders (10 and 40 stages), 12 short runs, a few MB.
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np  # noqa: E402

from rfr_common import default_work, col, last_plot, run_ng, run_rs  # noqa: E402

GATE = 1e-11
MUT = 1.0  # --selftest: relative error injected into the scaled capacitors
W = os.environ.get("RFR_WORK") or default_work("chk_scale")
os.makedirs(W, exist_ok=True)
REF = "reltol=1e-6 abstol=1e-15 vntol=1e-9"


def eng(x):
    return f"{x:.12g}"


def ladder(n, alpha, drive, shift_rows=0):
    """RC ladder, R = 100 ohm, C = alpha * 1 pF per stage, times scaled by alpha, tstep = alpha * 10 ps."""
    h = 10e-12 * alpha
    td = 0.2e-9 * alpha + shift_rows * h
    if drive == "pulse":
        src = f"PULSE(0 1 {eng(td)} {eng(0.1e-9 * alpha)} {eng(0.1e-9 * alpha)} {eng(1.5e-9 * alpha)} {eng(3e-9 * alpha)})"
    else:  # SIN(vo va freq td): the delay is applied through td, the frequency scales as 1/alpha
        src = f"SIN(0 1 {eng(1.3e9 / alpha)} {eng(td)})"
    els = [f"vs in 0 {src}"]
    prev = "in"
    for k in range(1, n + 1):
        els += [f"r{k} {prev} n{k} 100", f"c{k} n{k} 0 {eng(1e-12 * alpha * (MUT if alpha != 1.0 else 1.0))}"]
        prev = f"n{k}"
    tstop = 5e-9 * alpha + shift_rows * h
    return f"rc ladder {n} alpha={alpha:g} shift={shift_rows}\n" + "\n".join(els) + f"\n.tran {eng(h)} {eng(tstop)}\n.end\n"


def run_tran(deck, tag, ng=False):
    f = os.path.join(W, tag + ".sp")
    open(f, "w").write(deck)
    if ng:
        # tmax = tstep/20 like the suite's reference run
        m = re.search(r"\.tran (\S+) (\S+)", deck)
        d2 = deck.replace(m.group(0), f"{m.group(0)} 0 {float(m.group(1)) / 20:.6g}")
        open(f, "w").write(d2)
        return last_plot(run_ng(f, W, tag=tag + "_ng", options=REF)[0], "Transient")
    return last_plot(run_rs(f, W, tag=tag + "_rs")[0], "Transient")


def resample(p, names, t_grid):
    t = col(p, "time")
    return {n: np.interp(t_grid, t, col(p, n)) for n in names}


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        # a 1e-6 relative error in the scaled capacitors only must be rejected by the gate
        MUT = 1.0 + 1e-6
        base = run_tran(ladder(10, 1.0, "pulse"), "st_base")
        p = run_tran(ladder(10, 1e3, "pulse"), "st_scaled")
        nodes = [v for v in base["vars"] if v.startswith("v(n")]
        dev = max(np.abs(col(p, v) - col(base, v)).max() for v in nodes)
        print(f"SELFTEST: 1e-6 capacitor error -> max|v_alpha - v_1| = {dev:.2e} (gate {GATE:g}):", "rejected" if dev > GATE else "NOT rejected")
        sys.exit(0 if dev > GATE else 4)
    bad = 0
    print(f"gate {GATE:g} V (RustSpice only); ngspice column = control, adaptive steps, not gated")
    for n, drive in ((10, "pulse"), (40, "sin")):
        base = run_tran(ladder(n, 1.0, drive), f"base{n}")
        nodes = [v for v in base["vars"] if v.startswith("v(n")]
        sw = max(np.ptp(col(base, v)) for v in nodes)
        ngbase = run_tran(ladder(n, 1.0, drive), f"base{n}", ng=True)
        print(f"-- ladder {n} stages, {drive} drive: {base['data'].shape[0]} rows, largest node swing {sw:.3f} V")
        if sw < 0.05:
            print("   VACUOUS: waveforms do not move")
            bad += 1
        # (i) time-scale invariance
        for alpha in (1e-3, 1e3, 1e6):
            p = run_tran(ladder(n, alpha, drive), f"s{n}_{alpha:g}")
            if p["data"].shape != base["data"].shape:
                print(f"   alpha={alpha:g}: row count {p['data'].shape[0]} != {base['data'].shape[0]}  VIOLATION")
                bad += 1
                continue
            dev = max(np.abs(col(p, v) - col(base, v)).max() for v in nodes)
            ngp = run_tran(ladder(n, alpha, drive), f"s{n}_{alpha:g}", ng=True)
            g = np.linspace(0, 5e-9, 501)
            a = resample(ngbase, nodes, g)
            b = resample(ngp, nodes, g * alpha)
            ngdev = max(np.abs(a[v] - b[v]).max() for v in nodes)
            ok = dev <= GATE
            bad += not ok
            print(f"   scale alpha={alpha:8g}: max|v_alpha - v_1| rustspice {dev:.2e}  [ngspice control {ngdev:.2e}]  {'ok' if ok else 'VIOLATION'}")
        # (ii) time-shift invariance (delay = 37 rows)
        m = 37
        p = run_tran(ladder(n, 1.0, drive, shift_rows=m), f"sh{n}")
        dev = max(np.abs(col(p, v)[m:] - col(base, v)[: len(col(base, v))]).max() for v in nodes)
        # rows 0..m-1 of the shifted run are the quiescent prefix
        pre = max(np.abs(col(p, v)[:m] - col(base, v)[0]).max() for v in nodes)
        ok = dev <= GATE and pre <= GATE
        bad += not ok
        print(f"   time shift of {m} rows: max|v_shifted(t+m*h) - v(t)| {dev:.2e}, quiescent prefix {pre:.2e}  {'ok' if ok else 'VIOLATION'}")
    print("RESULT:", f"FAIL ({bad} violations)" if bad else "PASS")
    sys.exit(1 if bad else 0)
