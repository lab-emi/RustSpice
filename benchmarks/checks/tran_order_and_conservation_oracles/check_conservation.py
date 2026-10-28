#!/usr/bin/env python3
"""Conservation and frequency oracles for RustSpice's transient integrator (trapezoid after one BDF-1 step).

A. acc_ord_lc_free_ring_energy_period: lossless parallel LC kicked by a 10 mA, 4 ns current pulse, then free ring.
   After the source has ended the circuit is closed and lossless, so
     C1  E = C v^2/2 + L i^2/2 is constant (trapezoid = implicit midpoint conserves every quadratic invariant of a linear
         system exactly; only roundoff may move it).  Independent of any simulator: it is a property of the circuit.
     C2  the sampled free ring obeys the 3-term recurrence v[n+1] + v[n-1] = 2 cos(theta) v[n] exactly, with the trapezoid
         closed form theta = 2 atan(w0 h / 2) (a-priori from the method, not from the simulator); the measured relative
         period error T_num/T0 - 1 with T0 = 2 pi sqrt(L C) (the PHYSICAL period) must equal (w0 h/2)/atan(w0 h/2) - 1 to
         1e-8 and shrink with slope 2.
     C3  the injected energy E_free (after the pulse) converges to the exact E (scipy DOP853, rtol 1e-13, split at the
         PULSE breakpoints) with slope 2.
B. acc_ord_rlc_sine_series: discrete energy balance of the driven series RLC.  With the trapezoid (Crank-Nicolson) rule the
   balance  sum h*avg(v_in)*avg(i) - sum h*R*avg(i)^2 = E_L + E_C  holds exactly step by step (products of step averages);
   RustSpice's first step is backward Euler, which is NOT energy-consistent.
     C4  from step 2 on the balance closes to 1e-9 of the supplied energy;
     C5  the first-step defect is reported (it is the O(h^2) start-up error that makes i(v1) approach order 2 from below).
ngspice (as-is, tmax = h; and reltol=1e-9, tmax = h) is run on the same deck for comparison and is only REPORTED.
Gates (exit 1): C1 drift <= 1e-10; C2 |measured - closed form| <= 1e-8 and pair slopes in [1.9, 2.1]; C3 pair slopes
(h <= 4*nominal) in [1.8, 2.2]; C4 residual <= 1e-9.     Usage: check_conservation.py [--json out.json]
"""
import json
import os
import sys
import warnings

import numpy as np

warnings.filterwarnings("ignore", category=RuntimeWarning)
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from tranoc_lib import *  # noqa: E402,F401
from tranoc_oracles import *  # noqa: E402,F401

PFX = "acc_ord_"
TIGHT = "reltol=1e-9 vntol=1e-12 abstol=1e-15"


def free_ring_stats(o, t, v, i):
    k0 = int(np.searchsorted(t, o.t_end - 1e-15))
    E = o.energy(v, i)[k0:]
    drift = float((E.max() - E.min()) / E.mean())
    return k0, float(E.mean()), drift


def lc_part(out, fails):
    text, meta = deck_meta(os.path.join(CASES, "accuracy", PFX + "lc_free_ring_energy_period.sp"))
    o = LcPulse(element_value(text, "L1"), element_value(text, "C1"), 10e-3, 1e-9, 1e-9, 1e-9, 1e-9)
    assert "PULSE(0 10m 1n 1n 1n 1n 1)" in text
    nominal, tstop = tran_card(text)
    steps = series_steps(meta)
    rows = []
    for h in steps:
        d = with_tstep(text, h)
        r = run_rs(d, f"lcc_rs_{h:g}")
        t, v, i = r["time"], r["v(n)"], r["i(l1)"]
        k0, Emean, drift = free_ring_stats(o, t, v, i)
        ve, ie = o.exact(t)
        Eex = float(o.energy(ve, ie)[k0:].mean())
        vf = v[k0:]
        c = float(np.sum((vf[2:] + vf[:-2]) * vf[1:-1]) / (2 * np.sum(vf[1:-1] ** 2)))
        res_rec = float(np.abs(vf[2:] + vf[:-2] - 2 * c * vf[1:-1]).max() / np.abs(vf).max())
        theta = float(np.arccos(c))
        T_num = 2 * np.pi * h / theta
        perr = T_num / o.T0 - 1
        x = o.w0 * h / 2
        perr_cf = x / np.arctan(x) - 1
        row = {"h": h, "rows": len(t), "free_rows": len(vf), "drift_rs": drift, "recurrence_resid": res_rec, "period_err_rs": float(perr),
               "period_err_closed_form": float(perr_cf), "E_inj_rel_err": Emean / Eex - 1, "E_exact": Eex}
        for tag, opt in (("asis", None), ("tight", TIGHT)):
            n = run_ng(d, f"lcc_ng{tag}_{h:g}", options=opt, tmax=h)
            tn = n["time"]
            kk = int(np.searchsorted(tn, o.t_end + 1e-12))
            En = o.energy(n["v(n)"], n["i(l1)"])[kk:]
            row[f"drift_ng_{tag}"] = float((En.max() - En.min()) / En.mean())
            # numerical period of ngspice from its zero crossings of v (linear interpolation) in the free ring
            vn = n["v(n)"][kk - 1:]
            tt = tn[kk - 1:]
            up = [tt[j] - vn[j] * (tt[j + 1] - tt[j]) / (vn[j + 1] - vn[j]) for j in range(len(vn) - 1) if vn[j] < 0 <= vn[j + 1]]
            row[f"period_err_ng_{tag}"] = float(np.mean(np.diff(up)) / o.T0 - 1) if len(up) >= 3 else None
        rows.append(row)
        # gates
        if drift > 1e-10:
            fails.append(f"C1 energy drift at h={h:g}: {drift:.2e} > 1e-10")
        if abs(perr - perr_cf) > 1e-8:
            fails.append(f"C2 period error h={h:g}: measured {perr:.9e} vs trapezoid closed form {perr_cf:.9e}")
    hs = [r["h"] for r in rows]
    s_per = loglog_slopes(hs, [r["period_err_rs"] for r in rows])
    if not (np.all(s_per >= 1.9) and np.all(s_per <= 2.1)):
        fails.append(f"C2 period-error slopes {np.round(s_per, 3).tolist()} outside [1.9, 2.1]")
    gate = [r for r in rows if r["h"] <= 4 * nominal * (1 + 1e-9)]
    s_en = loglog_slopes([r["h"] for r in gate], [abs(r["E_inj_rel_err"]) for r in gate])
    if not (np.all(s_en >= 1.8) and np.all(s_en <= 2.2)):
        fails.append(f"C3 injected-energy slopes {np.round(s_en, 3).tolist()} outside [1.8, 2.2]")
    print(f"== A. LC free ring: w0 = {o.w0:.6e} rad/s, T0 = {o.T0 * 1e9:.6f} ns, pulse ends {o.t_end * 1e9:.1f} ns, tstop {tstop * 1e9:.0f} ns")
    hdr = ["h", "free rows", "dE/E rs", "dE/E ng asis", "dE/E ng 1e-9", "T err rs", "T err closed form", "T err ng asis", "T err ng 1e-9", "E_inj err", "recurrence resid"]
    tab = [[f"{r['h']:.4g}", r["free_rows"], fmt(r["drift_rs"]), fmt(r["drift_ng_asis"]), fmt(r["drift_ng_tight"]), f"{r['period_err_rs']:+.6e}",
            f"{r['period_err_closed_form']:+.6e}", fmt(r["period_err_ng_asis"]), fmt(r["period_err_ng_tight"]), f"{r['E_inj_rel_err']:+.3e}", fmt(r["recurrence_resid"])] for r in rows]
    print(table(tab, hdr))
    print(f"   period-error slopes {np.round(s_per, 3).tolist()}; injected-energy slopes (h <= {4 * nominal:.4g}) {np.round(s_en, 3).tolist()}")
    out["lc"] = {"rows": rows, "period_slopes": s_per.tolist(), "energy_slopes": s_en.tolist(), "T0": o.T0}


def rlc_part(out, fails):
    text, meta = deck_meta(os.path.join(CASES, "accuracy", PFX + "rlc_sine_series.sp"))
    R, L, C = element_value(text, "R1"), element_value(text, "L1"), element_value(text, "C1")
    nominal, tstop = tran_card(text)
    rows = []
    for h in series_steps(meta):
        r = run_rs(with_tstep(text, h), f"rlcc_rs_{h:g}")
        t, vin, vc = r["time"], r["v(in)"], r["v(out)"]
        i = -r["i(v1)"]  # current delivered by the source into R1 (SPICE sign: i(v1) flows into the + terminal)
        assert np.allclose(np.diff(t), h, rtol=1e-9), "RustSpice grid is not uniform"
        E = 0.5 * L * i**2 + 0.5 * C * vc**2
        iavg = 0.5 * (i[1:] + i[:-1])
        sup = h * 0.5 * (vin[1:] + vin[:-1]) * iavg
        dis = h * R * iavg**2
        dE = E[1:] - E[:-1]
        step_resid = sup - dis - dE  # per step
        first = float(step_resid[0])
        tot_sup = float(np.sum(sup[1:]))
        resid_rest = float(np.abs(np.cumsum(step_resid[1:])).max() / tot_sup)
        rows.append({"h": h, "supplied_energy": tot_sup, "first_step_defect_rel": first / float(sup[1:3].sum()), "first_step_defect_J": first, "balance_resid_rel": resid_rest,
                     "max_step_resid_rel": float(np.abs(step_resid[1:]).max() / np.abs(sup[1:]).max())})
        if resid_rest > 1e-9:
            fails.append(f"C4 energy balance after the first step at h={h:g}: {resid_rest:.2e} > 1e-9")
    print(f"== B. driven RLC energy balance (trapezoid products of step averages), R={R:g} L={L:g} C={C:g}")
    tab = [[f"{r['h']:.4g}", fmt(r["supplied_energy"]), fmt(r["balance_resid_rel"]), fmt(r["max_step_resid_rel"]), fmt(r["first_step_defect_J"])] for r in rows]
    print(table(tab, ["h", "supplied J (steps>=2)", "balance resid (rel, steps>=2)", "max step resid (rel)", "first-step defect J"]))
    s = loglog_slopes([r["h"] for r in rows], [abs(r["first_step_defect_J"]) for r in rows])
    print(f"   first-step (BDF-1) energy defect scales with slope {np.round(s, 2).tolist()} in h")
    out["rlc_balance"] = {"rows": rows, "first_step_defect_slopes": s.tolist()}


def main():
    jpath = sys.argv[2] if sys.argv[1:2] == ["--json"] else None
    out, fails = {}, []
    lc_part(out, fails)
    rlc_part(out, fails)
    out["violations"] = fails
    for f in fails:
        print("VIOLATION:", f)
    if jpath:
        json.dump(out, open(jpath, "w"), indent=1)
    print("RESULT:", "FAIL (%d violations)" % len(fails) if fails else "OK")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
