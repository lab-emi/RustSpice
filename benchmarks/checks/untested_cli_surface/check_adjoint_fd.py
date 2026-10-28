#!/usr/bin/env python3
"""`rustspice adjoint` against two independent references (SUITE_SPEC 13.4: never trust one simulator).

Per fixture deck (cap_cli_adj_*.sp) and target node the script checks every sensitivity the CLI prints:

  oracle  an independent 50-digit mpmath Newton solve of the deck (textbook MNA, SPICE3 diode, level-1 MOSFET) with the
          exact implicit-function-theorem derivative dV/dtheta = -J^-1 dF/dtheta   (gate 1e-8 relative: the CLI prints 10 digits)
  fd      Richardson-extrapolated central finite difference of the tightly converged black-box ngspice
          (.options reltol=1e-9 vntol=1e-12 abstol=1e-15, relative steps 2e-3 and 1e-3)  (gate 1e-7 relative + 2e-10 V)

plus the CLI surface: unknown node / missing file / missing flag exit codes, case-insensitive node names, the ground node,
bit-identical output across repeats and RAYON_NUM_THREADS, and two KNOWN-GAP observations that must stay reproducible:

  adjoint-output-truncated  the CLI computes N sensitivities but prints only the first 15 without saying so (parameters are
                            enumerated R, then D, then M: with >= 15 resistors the diode IS is never shown)
  adjoint-sources-not-listed  V/I source values (supported by the library) are never requested by the CLI although the
                            oracle shows they are non-vacuous (this is the registry slug `adjoint-coverage`)

With --api-bin (the in-process harness cap_cli_adjoint_api.rs) the library-only surface is checked too: V1.dc / I1.dc source
sensitivities, diode N / AREA, MOS L / KP against the same two references, plus loud errors for capacitor / inductor / unknown
parameters and the all-or-nothing behaviour (`adjoint-coverage`: one bad spec discards every gradient).

Exit code 0 = no unexpected violation (known gaps are reported, `gap_closed` is announced); 1 = unexpected violation;
2 = environment problem (binary / ngspice missing => `unverified`, never a pass).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _clicommon import *  # noqa: E402,F403

PREFIX = "cap_cli_"
CASES = {
    "adj_diode_net": ["b", "c", "d"],
    "adj_mos_inv_follower": ["x", "s"],
    "adj_mos_tgate": ["x", "y"],
    "adj_wide_ladder_sources": ["m", "k", "n9"],
}
ORACLE_REL, ORACLE_ABS_V = 1e-8, 1e-13
FD_REL, FD_ABS_V = 1e-7, 2e-10
LOW_SIGNAL_V = 1e-3  # |theta * dV/dtheta| below this: FD is noise-limited, only the oracle gate applies
H1, H2 = 2e-3, 1e-3
CLI_PRINT_LIMIT = 15  # observed: `.take(15)` in the CLI


class Report:
    def __init__(self):
        self.rows = []
        self.bad = 0

    def add(self, name, status, detail):
        self.rows.append({"check": name, "status": status, "detail": detail})
        if status.startswith("FAIL"):
            self.bad += 1
        print(f"[{status:<19}] {name}: {detail}")


def fd_derivative(text, ckt, node, dev, param, th0, tmp, ngspice):
    def f(t):
        return ngspice_op(set_param(text, ckt, dev, param, t), tmp, ngspice=ngspice)[f"v({node})"]

    def central(h):
        return (f(th0 * (1 + h)) - f(th0 * (1 - h))) / (2 * th0 * h)

    d1, d2 = central(H1), central(H2)
    return (4 * d2 - d1) / 3, d1


def check_deck(rep, stem, targets, args, tmp):
    path = os.path.join(args.decks, PREFIX + stem + ".sp")
    text = open(path).read()
    ckt = parse_deck(text)
    worst = {"oracle": 0.0, "fd": 0.0}
    n_or = n_fd = n_low = 0
    for node in targets:
        rc, out, err, _ = run([args.bin, "adjoint", path, "--target-node", node], timeout=30)
        if rc != 0:
            rep.add(f"{stem}/{node}/run", "FAIL", f"rc={rc} {err[-200:]}")
            continue
        n_comp, grads = parse_adjoint_stdout(out)
        grads = {k: v * (1 + args.inject_error) for k, v in grads.items()}  # self-test of the gates' teeth
        orc = Oracle(ckt, node_gmin=0)
        orc.solve()
        ng_v = ngspice_op(text, tmp, tag="base", ngspice=args.ngspice)[f"v({node})"]
        d_v = abs(float(orc.v(node)) - ng_v)
        # any difference between the oracle's and ngspice's operating point (e.g. a junction law that differs far in
        # reverse bias) limits how closely ngspice's FD can confirm a sensitivity
        model_gap = d_v / max(1.0, abs(ng_v))
        if d_v > 1e-7:
            rep.add(f"{stem}/{node}/oracle_vs_ngspice_op", "FAIL", f"oracle V={float(orc.v(node))!r} ngspice V={ng_v!r}")
        fd_rel = max(FD_REL, 20 * model_gap)
        for key, g in grads.items():
            dev, param = key.split(".")
            e = next(e for e in ckt.elems if e.name.lower() == dev)
            th0 = float(orc.val(e, param))
            o = float(orc.sens(node, dev, param))
            S = abs(th0 * o)
            err_v = abs(th0 * (g - o))
            allowed = ORACLE_REL * S + ORACLE_ABS_V
            n_or += 1
            worst["oracle"] = max(worst["oracle"], err_v / S if S > 0 else 0.0)
            if err_v > allowed:
                rep.add(f"{stem}/{node}/{key}/oracle", "FAIL", f"adjoint={g!r} oracle={o!r} |theta*diff|={err_v:.3e} V > {allowed:.3e} V")
            if S < LOW_SIGNAL_V:
                n_low += 1
                continue
            fd, fd_plain = fd_derivative(text, ckt, node, dev, param, th0, tmp, args.ngspice)
            n_fd += 1
            e_fd = abs(th0 * (g - fd))
            e_fd_orc = abs(th0 * (o - fd))
            allowed_fd = fd_rel * S + FD_ABS_V
            worst["fd"] = max(worst["fd"], e_fd / S)
            if e_fd > allowed_fd:
                rep.add(f"{stem}/{node}/{key}/ngspice_fd", "FAIL", f"adjoint={g!r} fd={fd!r} diff={e_fd:.3e} V > {allowed_fd:.3e} V")
            if e_fd_orc > allowed_fd:
                rep.add(f"{stem}/{node}/{key}/oracle_vs_fd", "FAIL", f"oracle={o!r} fd={fd!r}: the oracle itself disagrees with ngspice")
    rep.add(
        f"{stem}",
        "PASS" if not any(r["status"].startswith("FAIL") and r["check"].startswith(stem) for r in rep.rows) else "FAIL",
        f"{n_or} sensitivities vs oracle (worst rel {worst['oracle']:.1e}), {n_fd} vs ngspice FD (worst rel {worst['fd']:.1e}), "
        f"{n_low} low-signal (|theta*dV/dtheta| < {LOW_SIGNAL_V:g} V) FD-skipped",
    )


def cli_surface(rep, args, tmp):
    d1 = os.path.join(args.decks, PREFIX + "adj_diode_net.sp")
    # (a) unknown node
    rc, out, err, _ = run([args.bin, "adjoint", d1, "--target-node", "zz"])
    ok = rc == 1 and "zz" in err and "not found" in err and "dV(" not in out and "panicked" not in err
    rep.add("cli/unknown_node_is_loud", "PASS" if ok else "FAIL", f"rc={rc} stderr={err.strip()[:90]!r}")
    # (b) missing file / missing flag
    rc, out, err, _ = run([args.bin, "adjoint", os.path.join(tmp.path, "nope.sp"), "--target-node", "c"])
    rep.add("cli/missing_file_is_loud", "PASS" if rc not in (0, 101, None) and err.strip() else "FAIL", f"rc={rc}")
    rc, out, err, _ = run([args.bin, "adjoint", d1])
    rep.add("cli/missing_flag_is_usage_error", "PASS" if rc == 2 and "target-node" in err else "FAIL", f"rc={rc}")
    # (c) case-insensitive node
    _, o1, _, _ = run([args.bin, "adjoint", d1, "--target-node", "c"])
    _, o2, _, _ = run([args.bin, "adjoint", d1, "--target-node", "C"])
    g1, g2 = parse_adjoint_stdout(o1)[1], parse_adjoint_stdout(o2)[1]
    rep.add("cli/node_name_case_insensitive", "PASS" if g1 and g1 == g2 else "FAIL", f"{len(g1)} gradients identical: {g1 == g2}")
    # (d) V(ground) is identically 0 and a node tied to a source is constant: every sensitivity is exactly 0
    for node, why in (("0", "ground"), ("gnd", "ground alias"), ("a", "node tied to V1")):
        _, o, _, _ = run([args.bin, "adjoint", d1, "--target-node", node])
        g = parse_adjoint_stdout(o)[1]
        ok = bool(g) and all(v == 0.0 for v in g.values())
        rep.add(f"cli/target_{why.replace(' ', '_')}_gives_zero", "PASS" if ok else "FAIL", f"max|grad|={max((abs(v) for v in g.values()), default=float('nan')):.1e}")
    # (e) determinism: the 10-digit gradient lines are identical across repeats and rayon thread counts
    sig = set()
    for thr in ("1", "2", "8", "1", "8"):
        env = dict(os.environ, RAYON_NUM_THREADS=thr)
        _, o, _, _ = run([args.bin, "adjoint", d1, "--target-node", "c"], env=env)
        sig.add(tuple(re.findall(r"dV\(.*", o)))
    rep.add("cli/output_deterministic_across_threads", "PASS" if len(sig) == 1 else "FAIL", f"{len(sig)} distinct outputs in 5 runs (1/2/8 threads)")


def known_gaps(rep, args, tmp):
    path = os.path.join(args.decks, PREFIX + "adj_wide_ladder_sources.sp")
    text = open(path).read()
    ckt = parse_deck(text)
    _, out, _, _ = run([args.bin, "adjoint", path, "--target-node", "m"])
    n_comp, grads = parse_adjoint_stdout(out)
    # --- gap 1: truncation. Invariant that SHOULD hold: every computed sensitivity is shown.
    violated = n_comp is not None and len(grads) < n_comp
    if violated and len(grads) == CLI_PRINT_LIMIT and "d1.is" not in grads:
        rep.add("gap/adjoint-output-truncated", "KNOWN_GAP_CONFIRMED", f"CLI says it computed {n_comp} sensitivities, prints {len(grads)} (R1..R15) and hides D1.is; no 'more' marker")
    elif violated:
        rep.add("gap/adjoint-output-truncated", "FAIL", f"truncation changed shape: computed {n_comp}, printed {len(grads)}")
    else:
        rep.add("gap/adjoint-output-truncated", "GAP_CLOSED", f"computed {n_comp}, printed {len(grads)}: update the registry/doc")
    # the hidden parameter is non-vacuous: its oracle sensitivity is a real, large number (so hiding it matters)
    orc = Oracle(ckt, node_gmin=0)
    orc.solve()
    th = float(orc.val(next(e for e in ckt.elems if e.name.lower() == "d1"), "is"))
    S = abs(th * float(orc.sens("m", "d1", "is")))
    rep.add("gap/adjoint-output-truncated/hidden_parameter_is_non_vacuous", "PASS" if S > 1e-3 else "FAIL", f"|IS*dV(m)/dIS| = {S:.3e} V")
    # --- gap 2: V and I sources are never offered although they matter
    listed_sources = [k for k in grads if k.split(".")[0] in ("v1", "i1")]
    s_v = float(orc.sens("m", "v1", "dc"))
    s_i = float(orc.sens("m", "i1", "dc"))
    nonvac = abs(s_v) > 1e-3 and abs(s_i) > 1.0
    rep.add("gap/adjoint-sources-not-listed/oracle_nonvacuous", "PASS" if nonvac else "FAIL", f"dV(m)/dV1 = {s_v:.6g} V/V, dV(m)/dI1 = {s_i:.6g} ohm")
    if not listed_sources:
        rep.add("gap/adjoint-sources-not-listed", "KNOWN_GAP_CONFIRMED", "CLI parameter list contains only R, D.is, M.w, M.vto; V1/I1 sensitivities (oracle-proven non-zero) cannot be requested")
    else:
        rep.add("gap/adjoint-sources-not-listed", "GAP_CLOSED", f"sources listed: {listed_sources}")


def api_run(binary, deck, node, specs):
    rc, out, err, _ = run([binary, deck, node] + specs, timeout=30)
    g = {m.group(1).lower(): float(m.group(2)) for m in re.finditer(r"grad (\S+) (\S+)", out)}
    msg = out.split("ERR ", 1)[1].strip() if "ERR " in out else None
    return rc, g, msg


def api_checks(rep, args, tmp):
    """Library surface the CLI never requests: V/I source values, diode N/AREA, MOS L/KP, and invalid specs (harness cap_cli_adjoint_api.rs)."""
    if not args.api_bin:
        rep.add("api", "UNVERIFIED", "pass --api-bin (build cap_cli_adjoint_api.rs, see its header) to check V/I/N/AREA/L/KP sensitivities and bad-spec errors")
        return
    tmpd = tmp.f("api.sp")
    tests = [
        ("adj_wide_ladder_sources", "m", lambda t: t.replace("D1 m k DM", "D1 m k DM AREA=1"), ["v1.dc", "i1.dc", "r1.r", "d1.is", "d1.n", "d1.area"]),
        ("adj_mos_inv_follower", "s", lambda t: t, ["m1.w", "m1.l", "m1.kp", "m1.vto", "m3.w", "m3.l", "m3.kp"]),
    ]
    for stem, node, edit, specs in tests:
        text = edit(open(os.path.join(args.decks, PREFIX + stem + ".sp")).read())
        open(tmpd, "w").write(text)
        ckt = parse_deck(text)
        rc, g, msg = api_run(args.api_bin, tmpd, node, specs)
        if rc != 0 or len(g) != len(specs):
            rep.add(f"api/{stem}/run", "FAIL", f"rc={rc} {msg}")
            continue
        orc = Oracle(ckt, node_gmin=0)
        orc.solve()
        worst, worst_fd, bad = 0.0, 0.0, 0
        for key in specs:
            dev, param = key.split(".")
            e = next(e for e in ckt.elems if e.name.lower() == dev)
            th0 = float(orc.val(e, param))
            o = float(orc.sens(node, dev, param))
            S = abs(th0 * o)
            err_v = abs(th0 * (g[key] - o))
            worst = max(worst, err_v / max(S, 1e-300))
            if err_v > ORACLE_REL * S + ORACLE_ABS_V:
                bad += 1
                rep.add(f"api/{stem}/{key}/oracle", "FAIL", f"api={g[key]!r} oracle={o!r}")
            if S >= LOW_SIGNAL_V:
                fd, _ = fd_derivative(text, ckt, node, dev, param, th0, tmp, args.ngspice)
                e_fd = abs(th0 * (g[key] - fd))
                worst_fd = max(worst_fd, e_fd / S)
                if e_fd > max(FD_REL, 2e-7) * S + FD_ABS_V:
                    bad += 1
                    rep.add(f"api/{stem}/{key}/ngspice_fd", "FAIL", f"api={g[key]!r} fd={fd!r}")
        rep.add(f"api/{stem}", "PASS" if not bad else "FAIL", f"{len(specs)} library-only sensitivities ({', '.join(specs)}) vs oracle (worst rel {worst:.1e}) and ngspice FD (worst rel {worst_fd:.1e})")
    # invalid specs are loud ...
    base = os.path.join(args.decks, PREFIX + "adj_wide_ladder_sources.sp")
    for spec, why in (("c1.c", "capacitor"), ("l1.l", "inductor"), ("d1.cjo", "unsupported diode parameter"), ("nope.r", "unknown device")):
        rc, g, msg = api_run(args.api_bin, base, "m", [spec])
        rep.add(f"api/bad_spec_{why.replace(' ', '_')}_is_loud", "PASS" if rc == 3 and msg and not g else "FAIL", f"rc={rc} {msg}")
    # ... but one bad spec among good ones discards every gradient (registry slug adjoint-coverage: 'aborts on one unsupported spec')
    rc, g, msg = api_run(args.api_bin, base, "m", ["r1.r", "d1.cjo", "v1.dc"])
    if rc == 3 and not g:
        rep.add("gap/adjoint-coverage/one_bad_spec_discards_all", "KNOWN_GAP_CONFIRMED", f"[r1.r, d1.cjo, v1.dc] -> no gradient at all ({msg})")
    elif g:
        rep.add("gap/adjoint-coverage/one_bad_spec_discards_all", "GAP_CLOSED", f"{len(g)} gradients returned")
    else:
        rep.add("gap/adjoint-coverage/one_bad_spec_discards_all", "FAIL", f"rc={rc} {msg}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default=DEFAULT_BIN)
    ap.add_argument("--ngspice", default=DEFAULT_NGSPICE)
    ap.add_argument("--decks", default=DECK_DIR, help="directory with the cap_cli_* fixture decks (default: benchmarks/cases/capability)")
    ap.add_argument("--api-bin", default=None, help="compiled cap_cli_adjoint_api example (library-only sensitivities and bad specs)")
    ap.add_argument("--cpus", default=None, help="pin to a CPU range like 16-31")
    ap.add_argument("--json", default=None)
    ap.add_argument("--inject-error", type=float, default=0.0, help="self-test: scale every printed gradient by (1+x); the gates must then fail")
    ap.add_argument("--strict", action="store_true", help="known gaps count as failures")
    args = ap.parse_args()
    pin_cpus(args.cpus)
    if not os.access(args.bin, os.X_OK) or shutil.which(args.ngspice) is None:
        print("unverified: rustspice binary or ngspice not available")
        return 2
    rep = Report()
    tmp = Tmp()
    try:
        for stem, targets in CASES.items():
            check_deck(rep, stem, targets, args, tmp)
        cli_surface(rep, args, tmp)
        known_gaps(rep, args, tmp)
        api_checks(rep, args, tmp)
    finally:
        tmp.close()
    n_gap = sum(1 for r in rep.rows if r["status"] == "KNOWN_GAP_CONFIRMED")
    print(f"\nsummary: {len(rep.rows)} checks, {rep.bad} unexpected failures, {n_gap} known gaps confirmed")
    if args.json:
        json.dump({"script": "check_adjoint_fd", "bad": rep.bad, "rows": rep.rows}, open(args.json, "w"), indent=1)
    return 1 if rep.bad or (args.strict and n_gap) else 0


if __name__ == "__main__":
    sys.exit(main())
