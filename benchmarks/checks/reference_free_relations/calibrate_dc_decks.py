#!/usr/bin/env python3
"""Reproduces every number quoted in the notes of the twin-block DC relation decks.

For each deck: RustSpice (frozen binary), ngspice-47 (tight options, black box) and an independent python dense
float64 MNA oracle (numpy + longdouble refinement, rfr_common.mna_solve) evaluate the same netlist; the relation
residuals |r - 1| are printed together with the solver tier.  Extra rows show (a) separate single-mesh runs WITHOUT the
sensing network (cross-check that the twin deck measures the circuit and not the sensing network), and (b) the
tier-control variants (disconnected padding copies) that move RustSpice from UnrolledSymbolic to FaerSupernodal.

Usage: calibrate_dc_decks.py [workdir]      (default: <tmpfs>/rfr/cal; rule 9: <= 2000 unknowns, a few MB)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np  # noqa: E402

from gen_dc_decks import block, reciprocity, superposition  # noqa: E402
from rfr_common import col, default_work, fmt, mesh_links, mna_solve, run_ng, run_rs  # noqa: E402

W = sys.argv[1] if len(sys.argv) > 1 else default_work("cal")
os.makedirs(W, exist_ok=True)


def evaluate(name, els, rels, gain, vref=None):
    d = f"{name}\n" + "\n".join(els) + "\n.op\n.end\n"
    f = os.path.join(W, name + ".sp")
    open(f, "w").write(d)
    rs_plots, st, _ = run_rs(f, W, tag=name + "_rs")
    ng_plots, _, _ = run_ng(f, W, tag=name + "_ng")
    rs, ng = rs_plots[-1], ng_plots[-1]
    orc, _ = mna_solve(d)
    r_rs = np.array([col(rs, f"v({r})")[0] - 1 for r in rels])
    r_ng = np.array([col(ng, f"v({r})")[0] - 1 for r in rels])
    r_or = np.array([orc[r] - 1 for r in rels])
    print(
        f"{name:34s} tier={st['solver_kind']:16s} dim={len(rs['vars']):5d}  max|r-1|: rustspice={abs(r_rs).max():.2e} "
        f"ngspice={abs(r_ng).max():.2e} python_oracle={abs(r_or).max():.2e}   (residual in V = |r-1|/{gain:g})"
    )
    return rs, ng, orc, r_rs / gain


def single_mesh(n, links, inj_node):
    els = block("a", n, links) + [f"ia 0 an{inj_node} DC 1"]
    return "single\n" + "\n".join(els) + "\n.op\n.end\n"


if __name__ == "__main__":
    print("== reciprocity")
    inj14 = ["3_4", "10_9", "0_5", "12_2"]
    inj22 = ["5_6", "14_13", "2_17"]
    for n, K, inj, pad, name in (
        (14, 4, inj14, 0, "reciprocity_randmesh14"),
        (22, 3, inj22, 0, "reciprocity_randmesh22"),
        (22, 3, inj22, 1, "reciprocity_randmesh22_faer"),
    ):
        els, rels, tr = reciprocity(n, K, 7, inj, 200, pad_copies=pad)
        rs, ng, orc, dV = evaluate(name, els, rels, 200)
        # relative asymmetry of the twin deck (residual / transfer voltage) and the separate-run cross-check
        v = [col(rs, f"v({tr[2 * i]})")[0] for i in range(len(rels))]
        print("    twin: rel. asymmetry per pair  =", ["%.1e" % (abs(d) / x) for d, x in zip(dV, v)], " transfers V:", ["%.3f" % x for x in v])
        links = mesh_links(n, 7, 1e-2, 1e2)
        asym = []
        for j in range(K):
            for k in range(j + 1, K):
                vals = []
                for src, meas in ((inj[j], inj[k]), (inj[k], inj[j])):
                    f = os.path.join(W, "single.sp")
                    open(f, "w").write(single_mesh(n, links, src))
                    plots, st, _ = run_rs(f, W, tag="single_rs")
                    vals.append(col(plots[-1], f"v(an{meas})")[0])
                asym.append(abs(vals[0] - vals[1]) / max(vals))
        print("    separate single-mesh runs (no sensing network), tier", st["solver_kind"], ": rel. asymmetry =", ["%.1e" % a for a in asym])
    print("== superposition")
    for n, probes, i3node, name in (
        (14, ["3_3", "7_11", "11_7", "12_2"], "7_4", "superposition_randmesh14"),
        (22, ["5_5", "11_19", "19_11", "20_2"], "11_7", "superposition_randmesh22_faer"),
    ):
        els, rels = superposition(n, 7, 1.0, probes, i3node)
        rs, ng, orc, dV = evaluate(name, els, rels, 1.0)
        vmax = max(abs(col(rs, v)[0]) for v in rs["vars"] if v.startswith("v(f"))
        print(f"    max |v| over copy f = {vmax:.3f} V -> relative residual {abs(dV).max() / vmax:.1e}")
