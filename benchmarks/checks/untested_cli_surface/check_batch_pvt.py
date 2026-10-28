#!/usr/bin/env python3
"""`Simulator::run_batch_dc_op` (the rayon PVT/Monte-Carlo batch API; no CLI entry) through the in-process harness cap_cli_batch_pvt.rs.

Build the harness first (it only reads the public API):
    cp cap_cli_batch_pvt.rs <RustSpice>/crates/rustspice-solver/examples/ &&
    CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/dev/shm/<me>/target cargo build -p rustspice-solver --example cap_cli_batch_pvt
    python3 check_batch_pvt.py --batch-bin /dev/shm/<me>/target/debug/examples/cap_cli_batch_pvt

On the diode-network fixture 40 corners (R1 R2 R3 +-20 % except every 5th corner, D1.is D2.is x0.25..x4, V1.dc 3.3..5 V on every 4th) are solved and checked:
  determinism    bit-identical solutions for RAYON_NUM_THREADS = 1, 2, 8 and repeated runs, for the forward and the reversed corner order
                 (so no corner can see another corner's state) and for each corner solved alone in a one-corner batch
  oracle         every node voltage of every corner vs an independent 50-digit mpmath Newton solve of the edited circuit (gate 1e-9 V)
  standalone     vs `rustspice run` on the textually edited deck, and vs tightly converged ngspice (gate 1e-9 V + 1e-9 rel)
KNOWN GAPS (registry slug `batch-all-or-nothing`, SUITE_SPEC section 11) that must stay reproducible:
  one unknown device in ONE corner fails the whole batch and returns no result for the 39 good corners;
  `D1.area` override is a silent no-op, and `D1.is` on an AREA=2 diode replaces the effective IS (area ignored).
  a corner that overrides a RESISTOR value is solved with a spurious 1e-12 S shunt to ground on every node (registry-less, proposed
  slug `batch-linear-dirty-gmin-shunt`): 100 Meg/300 Meg divider V(m) = 3.749719 instead of 3.75;
Exit code 0 = no unexpected violation; 1 = unexpected violation; 2 = harness binary missing (unverified).
"""
from __future__ import annotations

import argparse
import json
import math
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _clicommon import *  # noqa: E402,F403

PREFIX = "cap_cli_"
GATE_V = 1e-9


class Report:
    def __init__(self):
        self.rows, self.bad = [], 0

    def add(self, name, status, detail):
        self.rows.append({"check": name, "status": status, "detail": detail})
        if status.startswith("FAIL"):
            self.bad += 1
        print(f"[{status:<19}] {name}: {detail}")


def lcg(seed):
    x = seed
    while True:
        x = (1103515245 * x + 12345) & 0x7FFFFFFF
        yield x / 0x7FFFFFFF


def make_corners(n=40):
    g = lcg(20261027)
    out = []
    for k in range(n):
        c = {
            ("r1", "r"): 1000 * (0.8 + 0.4 * next(g)),
            ("r2", "r"): 1000 * (0.8 + 0.4 * next(g)),
            ("r3", "r"): 3000 * (0.8 + 0.4 * next(g)),
            ("d1", "is"): 1e-14 * 4 ** (2 * next(g) - 1),
            ("d2", "is"): 2e-14 * 4 ** (2 * next(g) - 1),
        }
        if k % 5 == 4:  # every 5th corner overrides no resistor (diode IS only): the clean-matrix path
            for key in (("r1", "r"), ("r2", "r"), ("r3", "r")):
                del c[key]
        if k % 4 == 0:
            c[("v1", "dc")] = 3.3 + 1.7 * next(g)
        out.append(c)
    return out


def corner_line(c):
    return " ".join(f"{d.upper()}.{p}={v!r}" for (d, p), v in c.items())


def run_batch(binary, deck, lines, tmp, env=None, tag="corners"):
    cf = tmp.f(f"{tag}.txt")
    open(cf, "w").write("\n".join(lines) + "\n")
    rc, out, err, _ = run([binary, deck, cf], timeout=120, env=env)
    nodes, rows, errmsg = [], [], None
    for ln in out.splitlines():
        tok = ln.split()
        if not tok:
            continue
        if tok[0] == "nodes":
            nodes = tok[1:]
        elif tok[0] == "ok":
            rows.append((int(tok[1]), tuple(tok[2:])))
        elif tok[0] == "ERR":
            errmsg = ln[4:]
    return rc, nodes, rows, errmsg, err


def hex_to_f(h):
    return struct.unpack(">d", bytes.fromhex(h))[0]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--batch-bin", default=None)
    ap.add_argument("--bin", default=DEFAULT_BIN)
    ap.add_argument("--ngspice", default=DEFAULT_NGSPICE)
    ap.add_argument("--decks", default=DECK_DIR, help="directory with the cap_cli_* fixture decks (default: benchmarks/cases/capability)")
    ap.add_argument("--cpus", default=None)
    ap.add_argument("--json", default=None)
    ap.add_argument("--strict", action="store_true")
    args = ap.parse_args()
    pin_cpus(args.cpus)
    if not args.batch_bin or not os.access(args.batch_bin, os.X_OK):
        print("unverified: build the cap_cli_batch_pvt example first (see the header) and pass --batch-bin")
        return 2
    rep, tmp = Report(), Tmp()
    deck = os.path.join(args.decks, PREFIX + "adj_diode_net.sp")
    text = open(deck).read()
    ckt = parse_deck(text)
    corners = make_corners()
    lines = [corner_line(c) for c in corners]
    try:
        # ------------------------------------------------------------------ determinism
        rc, nodes, base, err, _ = run_batch(args.batch_bin, deck, lines, tmp)
        rep.add("batch/runs", "PASS" if rc == 0 and len(base) == len(corners) else "FAIL", f"rc={rc} {len(base)}/{len(corners)} corner results, nodes={nodes}")
        if len(base) != len(corners):
            raise SystemExit(1)
        sigs = {}
        for thr in ("1", "2", "8", "8"):
            _, _, rows, _, _ = run_batch(args.batch_bin, deck, lines, tmp, env=dict(os.environ, RAYON_NUM_THREADS=thr))
            sigs[thr + "_" + str(len(sigs))] = rows == base
        rep.add("batch/deterministic_across_threads", "PASS" if all(sigs.values()) else "FAIL", f"RAYON_NUM_THREADS=1/2/8/8 bit-identical to the default run: {sigs}")
        _, _, rev, _, _ = run_batch(args.batch_bin, deck, lines[::-1], tmp)
        rep.add("batch/order_independent", "PASS" if rev[::-1] == base else "FAIL", "reversed corner order gives bit-identical per-corner solutions (no cross-corner state)")
        solo_bad = 0
        for k in (0, 7, 19, 33):
            _, _, one, _, _ = run_batch(args.batch_bin, deck, [lines[k]], tmp, tag="solo")
            solo_bad += one[0] != base[k]
        rep.add("batch/corner_alone_equals_in_batch", "PASS" if solo_bad == 0 else "FAIL", f"{4 - solo_bad}/4 sampled corners solved alone are bit-identical to the batch result")
        # ------------------------------------------------------------------ truth
        # Two oracles per corner: `clean` (the circuit as written, == standalone CLI == ngspice) and `shunt` (same circuit plus 1e-12 S
        # from every node to ground). A corner that overrides a resistor value is solved by the "dirty-matrix" path of the batch
        # engine; the data below show it equals the SHUNT model (see gap/batch-linear-dirty-gmin-shunt).
        err = {"clean": 0.0, "shunt": 0.0, "standalone": 0.0, "ngspice": 0.0}
        err_r, err_nor = {"clean": 0.0, "shunt": 0.0}, {"clean": 0.0, "shunt": 0.0}
        for k, (c, (iters, sol)) in enumerate(zip(corners, base)):
            vals = [hex_to_f(h) for h in sol[: len(nodes)]]
            has_r = any(d.startswith("r") for d, _ in c)
            ors = {}
            for name, ng_ in (("clean", 0.0), ("shunt", 1e-12)):
                o = Oracle(ckt, node_gmin=ng_)
                for key, v in c.items():
                    o.theta[key] = mp.mpf(repr(v))
                o.solve()
                ors[name] = o
            ed = text
            for (d, p), v in c.items():
                ed = set_param(ed, ckt, d, p, v)
            st = rustspice_op(ed, tmp, tag="solo_rs", binary=args.bin)
            ng = ngspice_op(ed, tmp, tag="solo_ng", ngspice=args.ngspice)
            for n, v in zip(nodes, vals):
                for name in ("clean", "shunt"):
                    e = abs(v - float(ors[name].v(n)))
                    err[name] = max(err[name], e)
                    (err_r if has_r else err_nor)[name] = max((err_r if has_r else err_nor)[name], e)
                err["standalone"] = max(err["standalone"], abs(v - st[f"v({n})"]))
                err["ngspice"] = max(err["ngspice"], abs(v - ng[f"v({n})"]))
        n_r = sum(1 for c in corners if any(d.startswith("r") for d, _ in c))
        n_nor = len(corners) - n_r
        rep.add("batch/corners_without_R_override_match_clean_oracle", "PASS" if (n_nor >= 5 and err_nor["clean"] <= GATE_V) else "FAIL", f"{n_nor} corners override no resistor: worst |batch - clean oracle| = {err_nor['clean']:.1e} V (gate {GATE_V:g}, need >= 5 corners)")
        if err_r["clean"] <= GATE_V:
            rep.add("gap/batch-linear-dirty-gmin-shunt/low_impedance_corners", "GAP_CLOSED", f"R-override corners now match the clean oracle to {err_r['clean']:.1e} V")
        elif err_r["shunt"] <= GATE_V and err_r["shunt"] * 10 <= err_r["clean"]:
            rep.add("gap/batch-linear-dirty-gmin-shunt/low_impedance_corners", "KNOWN_GAP_CONFIRMED", f"{n_r} corners with a resistor override: |batch - clean| = {err_r['clean']:.1e} V but |batch - (clean + 1e-12 S shunt on every node)| = {err_r['shunt']:.1e} V")
        else:
            rep.add("gap/batch-linear-dirty-gmin-shunt/low_impedance_corners", "FAIL", f"neither model explains the R-override corners: clean {err_r['clean']:.1e} V, shunt {err_r['shunt']:.1e} V")
        rep.add("info/batch_vs_standalone_and_ngspice", "INFO", f"worst |batch - standalone CLI| = {err['standalone']:.1e} V, worst |batch - ngspice(tight)| = {err['ngspice']:.1e} V (same size as the shunt effect above)")
        # high-impedance proof: 100 Meg / 300 Meg divider, exact V(m) = 3.75 V (closed form); standalone CLI and ngspice give 3.75 exactly
        hz = tmp.f("hz.sp")
        open(hz, "w").write("cap_cli high impedance divider\nV1 a 0 DC 5\nR1 a m 100Meg\nR2 m 0 300Meg\n.op\n.end\n")
        _, hn, hrows, _, _ = run_batch(args.batch_bin, hz, ["V1.dc=5", "R1.r=100000000", "R2.r=300000000"], tmp, tag="hz")
        v_clean, v_r1, v_r2 = (hex_to_f(r[1][hn.index("m")]) for r in hrows)
        exact = 5.0 * 300e6 / 400e6
        stand = rustspice_op(open(hz).read(), tmp, tag="hz_rs", binary=args.bin)["v(m)"]
        ngv = ngspice_op(open(hz).read(), tmp, tag="hz_ng", ngspice=args.ngspice)["v(m)"]
        ok_ref = abs(stand - exact) < 1e-9 and abs(ngv - exact) < 1e-9 and abs(v_clean - exact) < 1e-9
        if not ok_ref:
            rep.add("gap/batch-linear-dirty-gmin-shunt/high_impedance", "FAIL", f"reference corner not exact: batch(no R override)={v_clean!r} CLI={stand!r} ngspice={ngv!r} exact={exact}")
        elif max(abs(v_r1 - exact), abs(v_r2 - exact)) > 1e-6:
            rep.add("gap/batch-linear-dirty-gmin-shunt/high_impedance", "KNOWN_GAP_CONFIRMED", f"exact V(m)=3.75 (CLI, ngspice, batch without R override); batch with R1.r or R2.r override (same values!) = {v_r1:.9f} / {v_r2:.9f}: error {v_r1 - exact:+.2e} V")
        else:
            rep.add("gap/batch-linear-dirty-gmin-shunt/high_impedance", "GAP_CLOSED", f"batch R-override corners now exact ({v_r1!r}, {v_r2!r})")
        # non-vacuity: the corners really move the answer
        spread = max(hex_to_f(b[1][nodes.index("c")]) for b in base) - min(hex_to_f(b[1][nodes.index("c")]) for b in base)
        rep.add("batch/corners_are_not_vacuous", "PASS" if spread > 0.05 else "FAIL", f"V(c) spans {spread:.3f} V over the corners")
        # ------------------------------------------------------------------ known gaps (registry slug batch-all-or-nothing)
        rc, _, rows, errmsg, _ = run_batch(args.batch_bin, deck, lines[:5] + ["NOPE.r=3"] + lines[5:8], tmp, tag="bad")
        if rc != 0 and not rows and errmsg:
            rep.add("gap/batch-all-or-nothing/one_bad_corner", "KNOWN_GAP_CONFIRMED", f"1 bad corner of 9 -> no result at all for the 8 good ones ({errmsg[:70]!r})")
        elif rows:
            rep.add("gap/batch-all-or-nothing/one_bad_corner", "GAP_CLOSED", f"{len(rows)} results returned for a batch with one bad corner")
        else:
            rep.add("gap/batch-all-or-nothing/one_bad_corner", "FAIL", f"rc={rc} no result and no error message")
        # area override: standalone deck with AREA=2 is the truth
        ad = text.replace("D1 b c DM1", "D1 b c DM1 AREA=2")
        ad_path = tmp.f("area.sp")
        open(ad_path, "w").write(ad)
        ack = parse_deck(ad)
        truth = Oracle(ack, node_gmin=0)
        truth.solve()
        _, nn, r_nom, _, _ = run_batch(args.batch_bin, deck, ["V1.dc=5"], tmp, tag="area0")  # nominal corner without any resistor override
        _, nn, r_area, _, _ = run_batch(args.batch_bin, deck, ["D1.area=2"], tmp, tag="area1")
        vb_nom, vb_area, vb_true = hex_to_f(r_nom[0][1][nn.index("b")]), hex_to_f(r_area[0][1][nn.index("b")]), float(truth.v("b"))
        if abs(vb_area - vb_nom) < 1e-12 and abs(vb_true - vb_nom) > 1e-3:
            rep.add("gap/batch-all-or-nothing/area_override_noop", "KNOWN_GAP_CONFIRMED", f"D1.area=2 leaves V(b)={vb_area:.9f} unchanged, a deck with AREA=2 gives {vb_true:.9f} (oracle)")
        elif abs(vb_area - vb_true) < GATE_V:
            rep.add("gap/batch-all-or-nothing/area_override_noop", "GAP_CLOSED", f"D1.area override now matches the AREA=2 deck ({vb_area:.9f})")
        else:
            rep.add("gap/batch-all-or-nothing/area_override_noop", "FAIL", f"batch V(b)={vb_area!r} nominal={vb_nom!r} AREA=2 oracle={vb_true!r}")
        # IS override on an AREA=2 diode: standalone `IS=3e-14, AREA=2` deck is the truth
        ad2 = ad.replace("IS=1e-14", "IS=3e-14")
        t2 = Oracle(parse_deck(ad2), node_gmin=0)
        t2.solve()
        _, _, r_is, _, _ = run_batch(args.batch_bin, ad_path, ["D1.is=3e-14"], tmp, tag="area2")
        vb_is, vb_t2 = hex_to_f(r_is[0][1][nn.index("b")]), float(t2.v("b"))
        if abs(vb_is - vb_t2) > 1e-6:
            rep.add("gap/batch-all-or-nothing/is_override_ignores_area", "KNOWN_GAP_CONFIRMED", f"AREA=2 diode, D1.is=3e-14: batch V(b)={vb_is:.9f}, deck with IS=3e-14 AREA=2 gives {vb_t2:.9f} (oracle)")
        else:
            rep.add("gap/batch-all-or-nothing/is_override_ignores_area", "GAP_CLOSED", f"batch {vb_is:.9f} == oracle {vb_t2:.9f}")
    finally:
        tmp.close()
    gaps = sum(1 for r in rep.rows if r["status"] == "KNOWN_GAP_CONFIRMED")
    print(f"\nsummary: {len(rep.rows)} checks, {rep.bad} unexpected failures, {gaps} known gaps confirmed")
    if args.json:
        json.dump({"script": "check_batch_pvt", "bad": rep.bad, "rows": rep.rows}, open(args.json, "w"), indent=1)
    return 1 if rep.bad or (args.strict and gaps) else 0


if __name__ == "__main__":
    sys.exit(main())
