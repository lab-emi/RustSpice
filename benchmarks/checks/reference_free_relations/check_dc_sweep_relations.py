#!/usr/bin/env python3
"""Reference-free relations of the DC sweep path (RustSpice only; ngspice-tight is a black-box control).

 (a) a `.dc` point must equal an independent `.op` run at the same source value: 6-diode chain (25 points) and CMOS
     inverter voltage-transfer curve (34 points, includes the high-gain region);
 (b) up and down sweeps of the same VTC must agree at every shared source value (no hysteresis / state leakage between
     sweep points: the solution of this circuit is unique);
 (c) a `.dc` sweep of a LINEAR resistor mesh is affine in the swept source: all second differences vanish (checks the
     sweep path for stale factorizations; 14x14 mesh = UnrolledSymbolic; the 22x22 mesh is a KNOWN_GAP in the unrolled
     tier (linear-static-pivot-accuracy) and passes when three disconnected padding copies move it to FaerSupernodal).

Gates (declared per relation, observed values in the table): (a) diode chain 1e-10 V, VTC 1e-9 V (RustSpice declares
convergence at 1e-6 relative, so the observed 6e-12 / 3e-11 V are far inside it), (b) 1e-9 V, (c) 1e-10 V on second differences of node voltages
swinging >= 0.1 V.  --selftest solves the .op decks 1e-6 relative away from the sweep points and exits 0 only if (a) rejects it.
Exit code 1 on any unexpected violation, 3 when a registered known gap no longer reproduces, 0 otherwise.  Rule 9: <= 1000 unknowns, ~150 tiny runs, a few MB.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np  # noqa: E402

from rfr_common import default_work, col, fmt, last_plot, mesh_links, run_ng, run_rs  # noqa: E402

W = os.environ.get("RFR_WORK") or default_work("chk_dc")
os.makedirs(W, exist_ok=True)
bad = 0
MUT = 1.0  # --selftest: relative error injected into the .op source values


def run_deck(text, tag, tool="rs"):
    f = os.path.join(W, tag + ".sp")
    open(f, "w").write(text)
    if tool == "rs":
        return run_rs(f, W, tag=tag + "_rs")[0]
    return run_ng(f, W, tag=tag + "_ng")[0]


def dc_vs_op(name, body_fmt, sweep, probes, gate):
    """body_fmt(valstr) -> deck body without analysis card; sweep = (lo, hi, step)."""
    global bad
    lo, hi, st = sweep
    deck_dc = f"{name}\n" + body_fmt("0") + f"\n.dc vsw {lo} {hi} {st}\n.end\n"
    rs_dc = last_plot(run_deck(deck_dc, name + "_dc"), "DC")
    ng_dc = last_plot(run_deck(deck_dc, name + "_dc", "ng"), "DC")
    xs = col(rs_dc, rs_dc["vars"][0])
    worst, worst_ng, swing = 0.0, 0.0, 0.0
    for k, x in enumerate(xs):
        deck_op = f"{name}\n" + body_fmt(repr(float(x) * MUT)) + "\n.op\n.end\n"
        op_rs = last_plot(run_deck(deck_op, name + "_op"), "Operating")
        op_ng = last_plot(run_deck(deck_op, name + "_op", "ng"), "Operating")
        for p in probes:
            worst = max(worst, abs(col(rs_dc, p)[k] - col(op_rs, p)[0]))
            worst_ng = max(worst_ng, abs(col(ng_dc, p)[k] - col(op_ng, p)[0]))
    for p in probes:
        swing = max(swing, np.ptp(col(rs_dc, p)))
    ok = worst <= gate and swing > 0.1
    bad += not ok
    print(f"(a) {name:14s} {len(xs):3d} points, probe swing {swing:.3f} V: max|dc - op| rustspice {worst:.2e}  [ngspice control {worst_ng:.2e}]  gate {gate:g}  {'ok' if ok else 'VIOLATION'}")
    return rs_dc


def chain_body(val):
    els = [f"vsw in 0 {val}", "r1 in n1 100"]
    for k in range(1, 7):
        els.append(f"dd{k} n{k} {'0' if k == 6 else 'n' + str(k + 1)} dm")
    els.append(".model dm D(IS=1e-14 N=1)")
    return "\n".join(els)


def vtc_body(val):
    return "\n".join([
        "vdd vdd 0 3.3", f"vsw in 0 {val}", "mn out in 0 0 nm w=10u l=1u", "mp out in vdd vdd pm w=25u l=1u",
        ".model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)",
        ".model pm pmos(level=1 vto=-0.8 kp=40u gamma=0.5 phi=0.7 lambda=0.04)"])


def vtc_sweep_values(down):
    return (3.3, 0, -0.1) if down else (0, 3.3, 0.1)


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        MUT = 1.0 + 1e-6  # the .op decks are solved 1e-6 relative away from the sweep points: relation (a) must be rejected
        dc_vs_op("diode_chain6", chain_body, (0, 4.8, 0.2), [f"v(n{k})" for k in range(1, 7)], 1e-10)
        print("SELFTEST:", "rejected" if bad else "NOT rejected")
        sys.exit(0 if bad else 4)
    print("-- (a) .dc point == .op")
    dc_vs_op("diode_chain6", chain_body, (0, 4.8, 0.2), [f"v(n{k})" for k in range(1, 7)], 1e-10)
    up = dc_vs_op("inverter_vtc", vtc_body, (0, 3.3, 0.1), ["v(out)", "i(vdd)"], 1e-9)
    print("-- (b) up sweep == down sweep (inverter VTC)")
    down = last_plot(run_deck(f"vtc down\n{vtc_body('0')}\n.dc vsw 3.3 0 -0.1\n.end\n", "vtc_down"), "DC")
    xu, xd = col(up, up["vars"][0]), col(down, down["vars"][0])
    worst = 0.0
    for i, x in enumerate(xu):
        j = int(np.argmin(np.abs(xd - x)))
        assert abs(xd[j] - x) < 1e-9, (x, xd[j])
        for p in ("v(out)", "i(vdd)"):
            worst = max(worst, abs(col(up, p)[i] - col(down, p)[j]))
    ok = worst <= 1e-9
    bad += not ok
    print(f"(b) inverter_vtc   {len(xu)} shared points: max|up - down| {worst:.2e} (v(out), i(vdd))  gate 1e-9  {'ok' if ok else 'VIOLATION'}")
    print("-- (c) linear mesh .dc is affine in the swept source (second differences)")
    from gen_dc_decks import block  # noqa: E402

    def swept_mesh(n, pads):
        links = mesh_links(n, 7, 1e-2, 1e2)
        els = [f"r{a} an{b} an{c} {fmt(r)}" for a, b, c, r in links]
        corners = ["0_0", f"0_{n - 1}", f"{n - 1}_0", f"{n - 1}_{n - 1}"]
        els += [f"rp0 pad an{corners[0]} 0.01", f"rp1 an{corners[1]} 0 0.01", f"rp2 an{corners[2]} 0 0.01", f"rp3 an{corners[3]} 0 0.01",
                "vsw pad 0 0", f"i1 0 an{n // 2}_{n // 3} DC 0.5"]
        for k in range(pads):  # disconnected static copies: only the matrix size (solver tier) changes
            els += block(f"z{k}", n, links) + [f"iz{k} 0 z{k}n{n // 2}_{n // 3} DC 0.5"]
        return "\n".join(els)

    closed = 0
    # (n, padding copies, name, gate, expected-gap slug or None)
    for n, pads, name, gap in ((14, 0, "mesh14", None), (22, 0, "mesh22", None), (22, 3, "mesh22_pad3", None)):
        deck = f"{name}\n" + swept_mesh(n, pads) + "\n.dc vsw -1 1 0.1\n.end\n"
        fdeck = os.path.join(W, name + ".sp")
        open(fdeck, "w").write(deck)
        plots, st, _ = run_rs(fdeck, W, tag=name + "_rs")
        p = last_plot(plots, "DC")
        nodes = [v for v in p["vars"] if v.startswith("v(an")]
        d2 = max(np.abs(np.diff(col(p, v), 2)).max() for v in nodes)
        sw = max(np.ptp(col(p, v)) for v in nodes)
        ng = last_plot(run_deck(deck, name, "ng"), "DC")
        d2n = max(np.abs(np.diff(col(ng, v), 2)).max() for v in nodes)
        viol = d2 > 1e-10 or sw <= 0.1
        if gap and viol:
            verdict = f"KNOWN_GAP ({gap}, ngspice control 6 orders better)"
        elif gap:
            verdict, closed = f"GAP_CLOSED ({gap}) - update the documentation", closed + 1
        else:
            verdict, bad = ("VIOLATION", bad + 1) if viol else ("ok", bad)
        print(f"(c) {name:12s} tier {st['solver_kind']:16s} {p['data'].shape[0]} points, {len(nodes)} nodes, swing {sw:.3f} V: max 2nd difference rustspice {d2:.2e}  [ngspice control {d2n:.2e}]  gate 1e-10  {verdict}")
    print("RESULT:", f"FAIL ({bad} violations)" if bad else ("PASS (a known gap closed: exit 3)" if closed else "PASS"))
    sys.exit(1 if bad else (3 if closed else 0))
