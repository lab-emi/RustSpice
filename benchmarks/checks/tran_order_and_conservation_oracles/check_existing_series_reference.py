#!/usr/bin/env python3
"""Bound on the ngspice REFERENCE error of an EXISTING corpus series deck that has no engine oracle (SUITE_SPEC 13.4.1).

acc_tr_diode_halfwave_rail (1 kHz, 10 V half-wave rectifier into 1 k || 1 uF, D IS=1e-14 N=1, series 40u..1u, nominal 10u)
is judged by the engine only against the converged ngspice run (`reltol=1e-6 abstol=1e-15 vntol=1e-9`, tmax = member step / 20).
Here the same circuit equation is integrated independently:  C dv/dt = iD(vin - v) - v/R,  iD = IS (exp(vd/Vt) - 1) + GMIN vd
(scipy Radau rtol 1e-11, DOP853 cross-check), and RustSpice, ngspice as-is (tmax = h) and the reference run are scored against it.
The deck is only READ (the corpus is not modified).  Gate (exit 1): the reference error at every member h <= nominal is
<= tol.norm * swing / 20, and the two oracle integrations agree to 1e-9 of the swing.     Usage: check_existing_series_reference.py
"""
import os
import sys
import warnings

import numpy as np

warnings.filterwarnings("ignore", category=RuntimeWarning)
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from tranoc_lib import *  # noqa: E402,F401
from tranoc_oracles import *  # noqa: E402,F401

CORPUS = str(CASES / "accuracy" / "acc_tr_diode_halfwave_rail.sp")
REF = "reltol=1e-6 abstol=1e-15 vntol=1e-9"


def main():
    text, meta = deck_meta(CORPUS)
    assert "SIN(0 10 1k)" in text and "IS=1e-14" in text
    R, C = element_value(text, "R1"), element_value(text, "C1")
    nominal, tstop = tran_card(text)
    tol = float(meta["tol.norm"])
    steps = series_steps(meta)
    nVt = VT
    IS = 1e-14

    def rhs(t, x):
        vd = 10 * np.sin(2 * np.pi * 1e3 * t) - x[0]
        return [((IS * np.expm1(min(vd / nVt, 700)) + GMIN * vd) - x[0] / R) / C]

    def truth(tg, method="Radau", rtol=1e-11, atol=1e-14):
        s = solve_ivp(rhs, (0, tg[-1]), [0.0], method=method, rtol=rtol, atol=atol, t_eval=tg)
        assert s.success, s.message
        return s.y[0]

    tg = np.linspace(0, tstop, 2001)
    a, b = truth(tg), truth(tg, "DOP853", 1e-12, 1e-15)
    sc = float(abs(a - b).max() / np.ptp(a))
    sw = float(np.ptp(a))
    print(f"== acc_tr_diode_halfwave_rail (read-only): swing {sw:.4f} V, tol.norm {tol:g}, oracle self-agreement {sc:.2e}")
    fails = [] if sc <= 1e-9 else [f"oracle self-agreement {sc:.2e}"]
    rows, tab = [], []
    for h in steps:
        d = with_tstep(text, h)
        rs = run_rs(d, f"ex_rs_{h:g}")
        e_rs = float(abs(rs["v(out)"] - truth(rs["time"])).max())
        n1 = run_ng(d, f"ex_ng1_{h:g}", tmax=h)
        e_n1 = float(abs(n1["v(out)"] - truth(n1["time"])).max())
        n2 = run_ng(d, f"ex_ng2_{h:g}", options=REF, tmax=h / 20)
        e_n2 = float(abs(n2["v(out)"] - truth(n2["time"])).max())
        rows.append((h, e_rs, e_n1, e_n2))
        tab.append([f"{h:.4g}", len(rs["time"]), fmt(e_rs), fmt(e_n1), fmt(e_n2), fmt(e_n2 / sw)])
        if h <= nominal * (1 + 1e-9) and e_n2 > tol * sw / 20:
            fails.append(f"reference error {e_n2:.2e} at h={h:g} exceeds tol.norm*swing/20 = {tol * sw / 20:.2e}")
    print(table(tab, ["h", "rs rows", "rs err V", "ng asis err V", "ng ref err V", "ng ref err / swing"]))
    s = loglog_slopes([r[0] for r in rows[2:]], [r[1] for r in rows[2:]])
    print("   RustSpice slopes for h <= 10u:", np.round(s, 3).tolist())
    for f in fails:
        print("VIOLATION:", f)
    print("RESULT:", "FAIL (%d violations)" % len(fails) if fails else "OK")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
