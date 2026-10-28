#!/usr/bin/env python3
"""Reference-free relation: self-convergence order of the fixed-step transient (Richardson, RustSpice only).

For a smooth drive the BDF-1 + trapezoid scheme must converge with order 2 in tstep.  With the RustSpice solutions
y_h, y_h/2, y_h/4 on the common time grid (no reference simulator, no closed form needed) the observed order is

    p = log2( ||y_h - y_h/2|| / ||y_h/2 - y_h/4|| )          (max norm over the common grid, all probed nodes)

and must lie in [1.8, 2.2].  Circuits: (1) RC low-pass (tau 100 ps) with SIN drive, (2) series RLC (R 10, L 10 nH, C 4 pF,
Q = 5) with SIN drive, (3) 10-stage RC ladder with SIN drive.  The deliberately non-smooth PULSE edges are NOT used (order is limited by
the corner, see the fixed-step-accuracy gap).  ngspice is not involved (adaptive steps have no fixed order).
Gate: observed order in [1.8, 2.2] and the finest-grid difference > 1e-9 (non-vacuity: the three runs must differ).
The base step must be in the asymptotic range: the single RC stage gives p = 1.02, 1.42, 1.71, 1.85, 1.92 for base steps
80, 40, 20, 10, 5 ps (measured), so h0 = 5 ps (h/tau = 0.05) is used.
--selftest: feeds the same gate with a non-geometric step triple (h, h/2, h/3), which is not a Richardson sequence, and
exits 0 only if the gate rejects it.
Exit code 1 on violation.  Rule 9: 9 tiny runs.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np  # noqa: E402

from rfr_common import default_work, col, last_plot, run_rs  # noqa: E402

W = os.environ.get("RFR_WORK") or default_work("chk_order2")
os.makedirs(W, exist_ok=True)


def circuits():
    rc = ["vs in 0 SIN(0 1 1G)", "r1 in out 100", "c1 out 0 1p"]
    rlc = ["vs in 0 SIN(0 1 1G)", "r1 in a 10", "l1 a out 10n", "c1 out 0 4p"]
    lad = ["vs in 0 SIN(0 1 1G)"]
    prev = "in"
    for k in range(1, 11):
        lad += [f"r{k} {prev} n{k} 100", f"c{k} n{k} 0 1p"]
        prev = f"n{k}"
    return {"rc": (rc, ["v(out)"]), "rlc": (rlc, ["v(out)"]), "ladder10": (lad, ["v(n3)", "v(n10)"])}


def run(els, probes, h, tag, tstop=4e-9):
    deck = "convergence\n" + "\n".join(els) + f"\n.tran {h:.9g} {tstop:.9g}\n.end\n"
    f = os.path.join(W, tag + ".sp")
    open(f, "w").write(deck)
    p = last_plot(run_rs(f, W, tag=tag + "_rs")[0], "Transient")
    return col(p, "time"), {v: col(p, v) for v in probes}


if __name__ == "__main__":
    bad = 0
    h0 = 5e-12
    selftest = "--selftest" in sys.argv
    ratios = (1, 2, 3) if selftest else (1, 2, 4)
    print("circuit    order p (h=5p,2.5p,1.25p)  ||y_h - y_h/2||   ||y_h/2 - y_h/4||   verdict")
    for name, (els, probes) in circuits().items():
        runs = [run(els, probes, h0 / r, f"{name}_{i}") for i, r in enumerate(ratios)]
        t4 = runs[2][0]
        sel = lambda r: [np.interp(t4, r[0], r[1][v]) for v in probes]  # noqa: E731
        y = [sel(r) for r in runs]
        d1 = max(np.abs(a - b).max() for a, b in zip(y[0], y[1]))
        d2 = max(np.abs(a - b).max() for a, b in zip(y[1], y[2]))
        p = float(np.log2(d1 / d2)) if d2 > 0 else float("nan")
        ok = 1.8 <= p <= 2.2 and d2 > 1e-9
        bad += not ok
        print(f"{name:10s} {p:8.3f}                  {d1:12.3e}      {d2:12.3e}         {'ok' if ok else 'VIOLATION'}")
    if selftest:
        print("SELFTEST:", "gate rejects the non-geometric sequence" if bad else "gate did NOT reject it")
        sys.exit(0 if bad else 4)
    print("RESULT:", f"FAIL ({bad} violations)" if bad else "PASS")
    sys.exit(1 if bad else 0)
