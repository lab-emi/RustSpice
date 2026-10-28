#!/usr/bin/env python3
"""Reference-free relation: Kirchhoff's current law re-evaluated from the raw node voltages.

A diode-resistor mesh (n x n nodes, 50 ohm links, a diode from every node to ground, IS = 1e-14, N = 1, supply through a
10 ohm pad into node (0,0)) is solved by RustSpice (`run --raw`).  From the written node voltages alone the script
recomputes, with an independent Shockley law, the net current leaving every node:

    r_k = sum_links (v_k - v_j)/R + IS*(exp(v_k/VT) - 1)   [+ (v_00 - V)/Rpad at node (0,0)]

and reports it as an equivalent voltage error  e_k = |r_k| / G_k  with G_k = sum 1/R + IS*exp(v_k/VT)/VT (the size of the
Newton step that would remove the residual).  Gate: max_k e_k <= 1e-9 V and |v_rs - v_oracle| <= 1e-9 V for n in {4, 8, 16}
and V in {0.9, 1.5, 3.0}.  ngspice-tight is a black-box control; an independent python dense Newton (float64) is the oracle.

VT = 0.025864917007 V is the thermal voltage at 300.15 K that BOTH tools use (calibrated on their observable outputs: with
CODATA-2018 constants, VT = 0.025864925786, the same residual would be 7e-7 A).  Both tools also put gmin = 1e-12 S across
every diode (RFR_GMIN=0 shows it: without it the voltages differ from the oracle by 1.0e-10 / 4.7e-10 / 2.0e-9 V for n = 4 / 8 / 16 =
n^2 * 7.6e-13 A * 10 ohm; with it RustSpice is within 1.7e-10 V and ngspice within 5e-12 V of the oracle).
Discrimination is deliberately modest: RustSpice converges far below its declared 1e-6, so this gate only catches a
solution that is not a KCL solution (gross garbage, stale factorization, wrong stamp); it is a self-consistency check.
--selftest perturbs one node voltage by 1e-8 V and exits 0 only if the gate rejects it.
Exit code 1 on violation.  Rule 9: <= 257 unknowns, 18 runs, a few MB.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np  # noqa: E402

from rfr_common import default_work, col, last_plot, run_ng, run_rs  # noqa: E402

VT = 0.025864917007
IS, RL, RPAD = 1e-14, 50.0, 10.0
GMIN = float(os.environ.get('RFR_GMIN', '1e-12'))  # both tools put 1e-12 S across every diode (verified: see docstring)
GATE_EQ_V = 1e-9
W = os.environ.get("RFR_WORK") or default_work("chk_kcl")
os.makedirs(W, exist_ok=True)


def deck(n, V):
    els = [f"v1 pad 0 DC {V}", f"rpad pad m0_0 {RPAD:g}"]
    for i in range(n):
        for j in range(n):
            if j + 1 < n:
                els.append(f"rh{i}_{j} m{i}_{j} m{i}_{j + 1} {RL:g}")
            if i + 1 < n:
                els.append(f"rv{i}_{j} m{i}_{j} m{i + 1}_{j} {RL:g}")
            els.append(f"dd{i}_{j} m{i}_{j} 0 dm")
    els.append(f".model dm D(IS={IS:g} N=1)")
    return f"diode mesh {n}x{n} V={V}\n" + "\n".join(els) + "\n.op\n.end\n"


def laplacian(n):
    N = n * n
    G = np.zeros((N, N))
    idx = lambda i, j: i * n + j  # noqa: E731
    for i in range(n):
        for j in range(n):
            for (a, b) in ((i + 1, j), (i, j + 1)):
                if a < n and b < n:
                    p, q = idx(i, j), idx(a, b)
                    G[p, p] += 1 / RL; G[q, q] += 1 / RL; G[p, q] -= 1 / RL; G[q, p] -= 1 / RL
    G[0, 0] += 1 / RPAD
    return G


def kcl(n, v, V):
    """Residual vector r (A) and local conductance g (S) for node voltages v (flattened n*n)."""
    G = laplacian(n)
    b = np.zeros(n * n)
    b[0] = V / RPAD
    e = np.exp(v / VT)
    r = G @ v - b + IS * (e - 1.0) + GMIN * v
    g = np.diag(G) + IS * e / VT + GMIN  # diagonal conductance: the Newton denominator of the decoupled step
    return r, g


def oracle(n, V):
    """Dense float64 damped Newton on the nodal equations (independent of both simulators)."""
    G = laplacian(n)
    b = np.zeros(n * n)
    b[0] = V / RPAD
    v = np.zeros(n * n)
    for _ in range(500):
        e = np.exp(v / VT)
        F = G @ v - b + IS * (e - 1.0) + GMIN * v
        J = G + np.diag(IS * e / VT + GMIN)
        dv = np.linalg.solve(J, -F)
        s = np.max(np.abs(dv))
        v = v + dv * min(1.0, 0.1 / s) if s > 0.1 else v + dv
        if np.max(np.abs(F)) < 1e-15 and s < 1e-15:
            break
    return v


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        # perturb one RustSpice node voltage by 1e-8 V: the recomputed KCL residual must expose it
        n, V = 8, 1.5
        f = os.path.join(W, "st.sp")
        open(f, "w").write(deck(n, V))
        rs = last_plot(run_rs(f, W, tag="st_rs")[0], "Operating")
        v = np.array([col(rs, f"v(m{i}_{j})")[0] for i in range(n) for j in range(n)])
        r0, g0 = kcl(n, v, V)
        v[27] += 1e-8
        r1, g1 = kcl(n, v, V)
        e0, e1 = np.max(np.abs(r0) / g0), np.max(np.abs(r1) / g1)
        print(f"SELFTEST: equivalent voltage error {e0:.2e} -> {e1:.2e} after a 1e-8 V perturbation (gate {GATE_EQ_V:g}):", "rejected" if e1 > GATE_EQ_V else "NOT rejected")
        sys.exit(0 if e1 > GATE_EQ_V and e0 <= GATE_EQ_V else 4)
    bad = 0
    print(f"gate: equivalent voltage error <= {GATE_EQ_V:g} V and |v - oracle| <= {GATE_EQ_V:g} V (RustSpice); ngspice = control")
    print("  n   V[V]  max node V   RS: max|r|[A]  max e[V]  |v-orc|[V]    NG: max|r|[A]  max e[V]  |v-orc|[V]   verdict")
    for n in (4, 8, 16):
        for V in (0.9, 1.5, 3.0):
            f = os.path.join(W, f"m{n}_{V}.sp")
            open(f, "w").write(deck(n, V))
            rs = last_plot(run_rs(f, W, tag=f"rs{n}_{V}")[0], "Operating")
            ng = last_plot(run_ng(f, W, tag=f"ng{n}_{V}")[0], "Operating")
            orc = oracle(n, V)
            row = []
            ok = True
            for tool, p in (("rs", rs), ("ng", ng)):
                v = np.array([col(p, f"v(m{i}_{j})")[0] for i in range(n) for j in range(n)])
                r, g = kcl(n, v, V)
                eq, dv = np.max(np.abs(r) / g), np.max(np.abs(v - orc))
                row += [np.max(np.abs(r)), eq, dv]
                if tool == "rs":
                    ok = eq <= GATE_EQ_V and dv <= GATE_EQ_V and v.max() > 0.3
            bad += not ok
            print(f"  {n:2d}  {V:4.1f}  {orc.max():9.4f}   {row[0]:11.2e} {row[1]:9.2e} {row[2]:10.2e}   {row[3]:11.2e} {row[4]:9.2e} {row[5]:10.2e}   {'ok' if ok else 'VIOLATION'}")
    print("RESULT:", f"FAIL ({bad} violations)" if bad else "PASS")
    sys.exit(1 if bad else 0)
