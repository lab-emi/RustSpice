#!/usr/bin/env python3
"""Reference-free relation: a transient with DC-only drive must stay at its own operating point.

RustSpice only: v_tran(t) must equal v_op for every node and every row, for the SAME circuit solved with `.op` and
with `.tran`.  ngspice is a black-box control (its own tran-vs-op deviation is printed, it is not part of the gate).

Circuits: three DC-driven 5-stage CMOS inverter chains (Vin = 1.5 V, 1.65 V = trip point of the symmetric inverter,
1.8 V), VDD 3.3 V, 20 fF per stage, `.tran 10p 2n`  (same netlist as deck acc_rel_tran_rest_equals_op_cmos5
but with every stage probed, including the nodes at ~1e-9 V and the gain-region stages b4/b5), and a diode-resistor
rectifier-free divider (DC-driven RC + diode) as a second family.

Gate: |v_tran(t) - v_op| <= RELTOL*|v_op| + VNTOL for every node/row, with the declared RustSpice convergence
(reltol 1e-6, vntol 1e-10; `.options` are ignored by RustSpice).  Exit code 1 on violation, 2 on setup errors.
--selftest drives the transient 1e-5 relative away from the operating-point deck and exits 0 only if the gate rejects it.
Rule 9: <= 30 unknowns, 3 runs, a few MB.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np  # noqa: E402

from rfr_common import default_work, col, last_plot, run_ng, run_rs  # noqa: E402

RELTOL, VNTOL = 1e-6, 1e-10
W = os.environ.get("RFR_WORK") or default_work("chk_rest")
os.makedirs(W, exist_ok=True)

MODELS = [
    ".model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)",
    ".model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)",
]


def chain(tag, vin, nst=5):
    els = [f"vin{tag} {tag}0 0 DC {vin}"]
    prev = f"{tag}0"
    for k in range(1, nst + 1):
        n = f"{tag}{k}"
        els += [
            f"mp{tag}{k} {n} {prev} vdd vdd PMOD W=20u L=1u",
            f"mn{tag}{k} {n} {prev} 0 0 NMOD W=10u L=1u",
            f"cl{tag}{k} {n} 0 20f",
        ]
        prev = n
    return els


def cmos_deck(vins, card):
    els = ["vdd vdd 0 DC 3.3"]
    for t, v in zip("abc", vins):
        els += chain(t, v)
    return "cmos rest\n" + "\n".join(els + MODELS + [card, ".end"]) + "\n"


def diode_deck(card):
    # DC-driven RC + diode network: 1 V source, R-C-diode ladder (diodes conduct ~mA)
    els = ["v1 in 0 DC 1.2", "r1 in n1 330", "d1 n1 n2 dmod", "r2 n2 n3 220", "d2 n3 0 dmod", "c1 n1 0 1p", "c2 n2 0 2p", "c3 n3 0 1p",
           "r3 n2 0 5k", ".model dmod D(IS=1e-14 N=1.05)"]
    return "diode rest\n" + "\n".join(els + [card, ".end"]) + "\n"


def check(name, deck_tran, deck_op, nodes=None):
    ft, fo = os.path.join(W, name + "_t.sp"), os.path.join(W, name + "_o.sp")
    open(ft, "w").write(deck_tran)
    open(fo, "w").write(deck_op)
    rt = last_plot(run_rs(ft, W, tag=name + "_rs_t")[0], "Transient")
    ro = last_plot(run_rs(fo, W, tag=name + "_rs_o")[0], "Operating")
    ngt = last_plot(run_ng(ft, W, tag=name + "_ng_t", options="reltol=1e-6 abstol=1e-15 vntol=1e-9")[0], "Transient")
    ngo = last_plot(run_ng(fo, W, tag=name + "_ng_o")[0], "Operating")
    rows = []
    bad = 0
    for v in ro["vars"]:
        if not v.startswith("v(") or v == "v(vdd)":
            continue
        op = col(ro, v)[0]
        dev = np.abs(col(rt, v) - op).max()
        gate = RELTOL * abs(op) + VNTOL
        ng_dev = np.abs(col(ngt, v) - col(ngo, v)[0]).max()
        rs_vs_ng_op = abs(op - col(ngo, v)[0])
        ok = dev <= gate
        bad += not ok
        rows.append((v, op, dev, gate, ng_dev, rs_vs_ng_op, ok))
    swing_ok = sum(1 for r in rows if abs(r[1]) > 1e-3)
    print(f"-- {name}: {len(rows)} nodes ({swing_ok} with |v_op| > 1 mV), tran rows {rt['data'].shape[0]}")
    print("   node     v_op[V]        max|v_tran - v_op|   gate        ngspice tran-vs-op   |op_rs - op_ng|   verdict")
    for v, op, dev, gate, ngd, d2, ok in rows:
        print(f"   {v:7s} {op:14.9f} {dev:12.3e}   {gate:10.3e} {ngd:12.3e}   {d2:12.3e}   {'ok' if ok else 'VIOLATION'}")
    if swing_ok == 0:
        print("   VACUOUS: no node away from 0 V")
        return len(rows) + 1
    return bad


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        # the transient deck is driven 1e-5 relative away from the .op deck: the rest relation must be rejected
        n_bad = check("selftest", cmos_deck((1.5 * (1 + 1e-5), 1.65, 1.8), ".tran 10p 2n"), cmos_deck((1.5, 1.65, 1.8), ".op"))
        print("SELFTEST:", "rejected" if n_bad else "NOT rejected")
        sys.exit(0 if n_bad else 4)
    total = 0
    total += check("cmos5", cmos_deck((1.5, 1.65, 1.8), ".tran 10p 2n"), cmos_deck((1.5, 1.65, 1.8), ".op"))
    total += check("diode", diode_deck(".tran 5p 1n"), diode_deck(".op"))
    print("RESULT:", "FAIL (%d violations)" % total if total else "PASS")
    sys.exit(1 if total else 0)
