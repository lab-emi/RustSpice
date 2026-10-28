#!/usr/bin/env python3
"""Order of convergence of RustSpice's transient integrator against INDEPENDENT oracles, plus a bound on the error of
the ngspice reference that the differential suite uses (SUITE_SPEC 13.4.1).

For each acc_ord_*_series deck of benchmarks/cases/accuracy (RLC/sine, rectifier, LC kick, CMOS inverter) and every member of its
`bench.series.tstep` list:
  * RustSpice runs the deck at that step; its rows lie on the k*h grid, so the oracle is evaluated at exactly those times
    (no interpolation): err_rs(h) = max_t |v_rs - v_oracle|.
  * ngspice runs three ways at the same step: as-is (tmax = h), `reltol=1e-9` (tmax = h) and the suite REFERENCE mode
    (`reltol=1e-6 abstol=1e-15 vntol=1e-9`, tmax = h/20); the errors are vs the same oracle at ngspice's own rows.
Oracles (tranoc_oracles.py): RLC closed form (+ scipy DOP853 cross-check), rectifier scalar ODE via Lambert-W elimination of
the algebraic node (Radau rtol 1e-11, + DOP853 cross-check), LC piecewise DOP853 rtol 1e-13, inverter level-1 ODE (Radau, +
DOP853 cross-check).  Rule 8: the oracle and the code under test share no assumption: the oracles integrate the circuit
equations with scipy to 1e-11, the simulators solve the netlist.

Gates (exit 1 on any violation):
  G1 oracle self-agreement: two independent integrations of the oracle agree to <= 1e-9 (relative to the swing).
  G2 order: for the members h <= 2*nominal every consecutive log-log slope of err_rs is in [1.7, 2.3], the finest pair
     in [1.9, 2.1] and the least-squares slope in [1.8, 2.2].  (A BDF-1 integrator would give 1.0, an LTE-controlled one
     a flat curve.  The 1.7 floor is for the first-step BDF-1 start-up term in i(v1) of the RLC deck, which approaches
     slope 2 from below: 1.73, 1.89, 1.96 for h = 250p..31p.)
  G3 reference trust: for members h <= nominal, err(ngspice reference mode vs oracle) <= tol.norm * swing / 20, i.e. the
     reference is >= 20x tighter than the deck tolerance, so a `pass` or `fail` of the deck is RustSpice's.
  G4 deck tolerance consistency: err_rs(nominal) <= tol.norm * swing (the engine verdict agrees with the oracle verdict).
Usage: check_order_convergence.py [--json out.json] [deckstem ...]       (RUSTSPICE / NGSPICE env override the binaries)
"""
import json
import os
import sys

import warnings

import numpy as np

warnings.filterwarnings("ignore", category=RuntimeWarning)  # rejected trial stages of the ODE solvers overflow exp()

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from tranoc_lib import *  # noqa: E402,F401
from tranoc_oracles import *  # noqa: E402,F401

PFX = "acc_ord_"
REF = "reltol=1e-6 abstol=1e-15 vntol=1e-9"
TIGHT = "reltol=1e-9 vntol=1e-12 abstol=1e-15"


def build_rlc(text):
    o = RlcSine(element_value(text, "R1"), element_value(text, "L1"), element_value(text, "C1"), 1.0, 100e6)
    assert "SIN(0 1 100meg)" in text

    def truth(t):
        i, v = o.exact(t)
        return {"v(out)": v, "i(v1)": -i}

    def selfcheck(t):
        i, v = o.exact(t)
        i2, v2 = o.dop853(t)
        return max(abs(v - v2).max() / np.ptp(v), abs(i - i2).max() / np.ptp(i))

    return truth, selfcheck


def build_rect(text):
    o = Rectifier(3.0, 10e6, element_value(text, "R1"), element_value(text, "R2"), element_value(text, "C1"), 1e-14)
    assert "SIN(0 3 10meg)" in text and "IS=1e-14" in text

    def truth(t):
        v = o.solve(t)
        return {"v(out)": v, "v(a)": o.anode(t, v)}

    def selfcheck(t):
        a = o.solve(t, "Radau", 1e-11, 1e-16)
        b = o.solve(t, "DOP853", 1e-12, 1e-17)
        return abs(a - b).max() / np.ptp(a)

    return truth, selfcheck


def build_lc(text):
    o = LcPulse(element_value(text, "L1"), element_value(text, "C1"), 10e-3, 1e-9, 1e-9, 1e-9, 1e-9)
    assert "PULSE(0 10m 1n 1n 1n 1n 1)" in text  # PULSE(v1 v2 td tr tf pw per)

    def truth(t):
        v, i = o.exact(t)
        return {"v(n)": v, "i(l1)": i}

    def selfcheck(t):
        v, i = o.exact(t)
        # independent: the same system with a different integrator and tolerance, no breakpoint splitting
        f = lambda tt, x: [(o.isrc(tt) - x[1]) / o.C, x[0] / o.L]
        s = solve_ivp(f, (0, t[-1]), [0, 0], method="Radau", rtol=1e-12, atol=1e-18, t_eval=t, max_step=2e-11)
        return max(abs(v - s.y[0]).max() / np.ptp(v), abs(i - s.y[1]).max() / np.ptp(i))

    return truth, selfcheck


def build_inv(text):
    kw = dict(VDD=3.3, VIN_off=1.65, VIN_amp=1.65, f=500e6, CL=element_value(text, "CL"), wn=10e-6, wp=20e-6, ln=1e-6, lp=1e-6,
              kpn=120e-6, kpp=60e-6, vtn=0.7, vtp=-0.7, lam=0.02)
    assert "SIN(1.65 1.65 500meg)" in text and "W=10u L=1u" in text and "W=20u L=1u" in text
    o = Inverter(**kw)

    def truth(t):
        v = o.solve(t)
        return {"v(out)": v, "i(vdd)": o.supply_current(t, v)}

    def selfcheck(t):
        a = o.solve(t, "Radau", 1e-11, 1e-14)
        b = o.solve(t, "DOP853", 1e-12, 1e-15)
        return abs(a - b).max() / np.ptp(a)

    return truth, selfcheck


DECKS = {
    "rlc_sine_series": build_rlc,
    "diode_halfwave_rc_series": build_rect,
    "lc_free_ring_energy_period": build_lc,
    "cmos_inverter_l1_series": build_inv,
}


def run_deck(stem, out):
    path = os.path.join(CASES, "accuracy", PFX + stem + ".sp")
    text, meta = deck_meta(path)
    probes = [p.strip() for p in meta["probes"].split(",")]
    tol = float(meta["tol.norm"])
    nominal, tstop = tran_card(text)
    steps = series_steps(meta)
    truth, selfcheck = DECKS[stem](text)
    fails = []
    rows = []
    sw = None
    for h in steps:
        d = with_tstep(text, h)
        rs = run_rs(d, f"{stem}_rs_{h:g}")
        tr = truth(rs["time"])
        if sw is None:
            sw = {p: float(np.ptp(tr[p])) for p in probes}
        e_rs = {p: float(abs(rs[p] - tr[p]).max()) for p in probes}
        res = {"h": h, "rows_rs": len(rs["time"]), "err_rs": e_rs}
        for tag, opt, tmax in (("asis", None, h), ("tight1e-9", TIGHT, h), ("ref", REF, h / 20)):
            ng = run_ng(d, f"{stem}_ng{tag}_{h:g}", options=opt, tmax=tmax)
            tn = truth(ng["time"])
            res["err_ng_" + tag] = {p: float(abs(ng[p] - tn[p]).max()) for p in probes}
            res["rows_ng_" + tag] = len(ng["time"])
        rows.append(res)
    # G1
    tg = np.linspace(0, tstop, 801) if stem != "lc_free_ring_energy_period" else np.linspace(0, tstop, 1601)
    sc = float(selfcheck(tg))
    if not sc <= 1e-9:
        fails.append(f"G1 oracle self-agreement {sc:.2e} > 1e-9")
    # G2
    gate = [r for r in rows if r["h"] <= 2 * nominal * (1 + 1e-9)]
    slopes = {}
    for p in probes:
        hs = [r["h"] for r in gate]
        es = [r["err_rs"][p] for r in gate]
        pair = loglog_slopes(hs, es)
        ls = float(np.polyfit(np.log(hs), np.log(es), 1)[0])
        slopes[p] = {"pairs": [float(x) for x in pair], "lsq": ls}
        if not (np.all(pair >= 1.7) and np.all(pair <= 2.3) and 1.9 <= pair[-1] <= 2.1 and 1.8 <= ls <= 2.2):
            fails.append(f"G2 order {p}: pair slopes {np.round(pair, 3).tolist()} lsq {ls:.3f}")
    # G3 / G4
    for r in rows:
        if r["h"] <= nominal * (1 + 1e-9):
            for p in probes:
                if r["err_ng_ref"][p] > tol * sw[p] / 20:
                    fails.append(f"G3 reference error {p} at h={r['h']:g}: {r['err_ng_ref'][p]:.2e} > {tol * sw[p] / 20:.2e}")
        if abs(r["h"] - nominal) < 1e-9 * nominal:
            for p in probes:
                if r["err_rs"][p] > tol * sw[p]:
                    fails.append(f"G4 nominal error {p}: {r['err_rs'][p]:.2e} > tol.norm*swing {tol * sw[p]:.2e}")
    # report
    print(f"== {stem}: nominal {nominal:g} s, tstop {tstop:g} s, tol.norm {tol:g}, swing " + ", ".join(f"{p} {sw[p]:.4g}" for p in probes))
    hdr = ["h", "rows"] + [f"rs {p}" for p in probes] + [f"ng asis {p}" for p in probes] + [f"ng 1e-9 {p}" for p in probes] + [f"ng ref {p}" for p in probes]
    tab = []
    for r in rows:
        tab.append([f"{r['h']:.4g}", r["rows_rs"]] + [fmt(r["err_rs"][p]) for p in probes] + [fmt(r["err_ng_asis"][p]) for p in probes]
                   + [fmt(r["err_ng_tight1e-9"][p]) for p in probes] + [fmt(r["err_ng_ref"][p]) for p in probes])
    print(table(tab, hdr))
    for p in probes:
        print(f"   slopes {p}: pairs {np.round(slopes[p]['pairs'], 3).tolist()} lsq {slopes[p]['lsq']:.3f}  (gate set h <= {2 * nominal:.4g})")
    print(f"   oracle self-agreement {sc:.2e}")
    out[stem] = {"nominal": nominal, "tol_norm": tol, "swing": sw, "rows": rows, "slopes": slopes, "oracle_selfcheck": sc, "violations": fails}
    for f in fails:
        print("   VIOLATION:", f)
    return fails


def main():
    args = sys.argv[1:]
    jpath = None
    if args[:1] == ["--json"]:
        jpath = args[1]
        args = args[2:]
    stems = args or list(DECKS)
    out = {}
    nfail = 0
    for s in stems:
        nfail += len(run_deck(s, out))
    if jpath:
        json.dump(out, open(jpath, "w"), indent=1)
    print("RESULT:", "FAIL (%d violations)" % nfail if nfail else "OK")
    sys.exit(1 if nfail else 0)


if __name__ == "__main__":
    main()
