#!/usr/bin/env python3
"""`rustspice run --backend {cpu,metal,cuda-tile,cuda-simt,wasm}`: equivalence, honesty and CLI strictness.

Equivalence (gating): every backend must reproduce `--backend cpu` on op, dc-sweep and transient decks
(raw files, every vector/row; gate |diff| <= 1e-9 + 1e-6*|ref|, same row count and identical time/sweep axis within 1e-12 rel).
The `cpu` reference itself is checked for run-to-run and RAYON_NUM_THREADS bit-determinism. The wasm backend only switches to
its double-float path at >= 1024 nonlinear devices, so two generated decks (1100 diodes, 1100 MOSFETs) are added.

Honesty / strictness (KNOWN-GAP observations, each must stay reproducible; `GAP_CLOSED` is announced when fixed):
  backend-label-not-executor   the banner names Apple Metal / NVIDIA CUDA / wasm-WebGPU although every arithmetic path is the
                               host CPU. Evidence: (1) platform is not Darwin for `metal`; (2) NO CUDA context is ever created by the
                               process (nvidia-smi --query-compute-apps polled while a ~0.5 s run executes; positive control: a torch
                               process holding a context IS listed); (3) bit-identical raw files to `cpu` on every small deck.
  stats-lacks-executor         `--stats` JSON has no backend/executor field, so the executor cannot be asserted by a program.
  cli-backend-typo-silent-cpu  `--backend gpu|cuda|<empty>` exits 0 and silently runs on cpu.
  cli-codegen-option-silent    `codegen --model bogus --target nonsense` exits 0 and emits the default mosfet-l1/msl kernel.

Exit code 0 = no unexpected violation; 1 = unexpected violation; 2 = binary/ngspice-independent prerequisites missing (unverified).
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import platform
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _clicommon import *  # noqa: E402,F403

BACKENDS = ["cpu", "metal", "cuda-tile", "cuda-simt", "wasm"]
GATE_ABS, GATE_REL = 1e-9, 1e-6
PREFIX = "cap_cli_"
FIXTURES = ["backend_nand_chain_op", "backend_diode_or_dc", "backend_nand_chain_tran", "adj_diode_net", "adj_mos_inv_follower", "adj_mos_tgate"]


class Report:
    def __init__(self):
        self.rows, self.bad = [], 0

    def add(self, name, status, detail):
        self.rows.append({"check": name, "status": status, "detail": detail})
        if status.startswith("FAIL"):
            self.bad += 1
        print(f"[{status:<19}] {name}: {detail}")


def write(path, lines):
    open(path, "w").write("\n".join(lines) + "\n")
    return path


def gen_decks(tmp: Tmp):
    """Generated decks. The wasm backend switches to its double-float evaluator at >= 1024 nonlinear devices."""
    out = {}
    n = 1100
    L = [f"cap_cli generated diode ladder with {n} diodes", "V1 n0 0 DC 5"]
    for i in range(1, n + 1):
        L += [f"R{i} n{i-1} n{i} 10", f"D{i} n{i} 0 DM"]
    out["gen_diode_ladder_1100"] = write(tmp.f("gen_diode_ladder_1100.sp"), L + [".model DM D(IS=1e-14 N=1)", ".op", ".end"])
    L = [f"cap_cli generated array of {n} common-source cells (bulk = source = 0 V)", "V1 vdd 0 DC 3.3", "V2 g 0 DC 1.4"]
    for i in range(1, n + 1):
        L += [f"R{i} vdd d{i} 10k", f"M{i} d{i} g 0 0 NM W={10 + 0.01 * i:.4f}u L=1u"]
    out["gen_mos_array_1100"] = write(tmp.f("gen_mos_array_1100.sp"), L + [".model NM NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.04 GAMMA=0.4 PHI=0.7)", ".op", ".end"])
    for n in (8, 1100):  # body-biased source followers: vbs = -Vs < 0
        L = [f"cap_cli generated array of {n} NMOS source followers (body effect)", "V1 vdd 0 DC 3.3", "V2 g 0 DC 2.6"]
        for i in range(1, n + 1):
            L += [f"M{i} vdd g s{i} 0 NM W=20u L=1u", f"R{i} s{i} 0 10k"]
        out[f"gen_follower_{n}"] = write(tmp.f(f"gen_follower_{n}.sp"), L + [".model NM NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.04 GAMMA=0.45 PHI=0.7)", ".op", ".end"])
    # mechanism controls: the same netlists with the suspected cause removed must make metal agree with cpu again
    out["gen_follower_8_gamma0"] = write(
        tmp.f("gen_follower_8_gamma0.sp"),
        [f"cap_cli control: 8 NMOS source followers with GAMMA=0", "V1 vdd 0 DC 3.3", "V2 g 0 DC 2.6"]
        + [x for i in range(1, 9) for x in (f"M{i} vdd g s{i} 0 NM W=20u L=1u", f"R{i} s{i} 0 10k")]
        + [".model NM NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.04 GAMMA=0 PHI=0.7)", ".op", ".end"],
    )
    out["gen_level1_divider"] = write(
        tmp.f("gen_level1_divider.sp"),
        ["cap_cli control: the LEVEL=2 stage with LEVEL=1", "V1 vdd 0 DC 3.3", "V2 g 0 DC 0.9", "R1 vdd d 20k", "M1 d g 0 0 NM W=10u L=1u",
         ".model NM NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.04)", ".op", ".end"],
    )
    out["gen_level2_divider"] = write(
        tmp.f("gen_level2_divider.sp"),
        ["cap_cli LEVEL=2 (EKV in RustSpice) common-source stage", "V1 vdd 0 DC 3.3", "V2 g 0 DC 0.9", "R1 vdd d 20k", "M1 d g 0 0 NM W=10u L=1u",
         ".model NM NMOS(LEVEL=2 VTO=0.7 KP=120u LAMBDA=0.04)", ".op", ".end"],
    )
    return out


# (deck, backend) -> slug of the KNOWN defect expected to make the backend deviate from cpu.
#   backend-df64-mos-model-mismatch: the double-float MOSFET evaluator (metal; wasm at >= 1024 nonlinear devices) has no body effect
#   (GAMMA/PHI ignored, gmbs = 0) and no EKV branch (LEVEL>=2 evaluated as Level 1); the cpu path implements both.
GAP = "backend-df64-mos-model-mismatch"
EXPECTED_GAPS = {
    ("backend_nand_chain_op", "metal"): GAP,
    ("backend_nand_chain_tran", "metal"): GAP,
    ("adj_mos_inv_follower", "metal"): GAP,
    ("adj_mos_tgate", "metal"): GAP,
    ("gen_follower_8", "metal"): GAP,
    ("gen_follower_1100", "metal"): GAP,
    ("gen_follower_1100", "wasm"): GAP,
    ("gen_level2_divider", "metal"): GAP,
}
NO_TRUTH = {"gen_level2_divider"}  # LEVEL=2 means EKV only inside RustSpice: ngspice is not an arbiter, backends must simply agree
TRAN = {"backend_nand_chain_tran"}  # no op-oracle: the gap is attributed through the identical device cards of backend_nand_chain_op


def truth_agrees(vec_cpu_or_backend, truth, floor=1e-7, rel=1e-6):
    """max excess of |a-truth| over (floor + rel*|truth|) for the vectors both have; <=1 means agreement."""
    worst = 0.0
    for k, t in truth.items():
        if k in vec_cpu_or_backend:
            worst = max(worst, abs(vec_cpu_or_backend[k] - t) / (floor + rel * abs(t)))
    return worst


def run_backend(binary, deck, backend, tmp, tag, env=None):
    raw, stats = tmp.f(f"{tag}.raw"), tmp.f(f"{tag}.json")
    for p in (raw, stats):
        if os.path.exists(p):
            os.unlink(p)
    rc, out, err, wall = run([binary, "run", deck, "--backend", backend, "--raw", raw, "--stats", stats], timeout=60, env=env)
    res = {"rc": rc, "out": out, "err": err, "raw": None, "stats": None}
    if os.path.exists(raw):
        res["raw"] = read_raw(raw)
    if os.path.exists(stats):
        res["stats"] = json.load(open(stats))
    m = re.search(r"\(Backend: ([^,]+),", out)
    res["label"] = m.group(1) if m else None
    return res


def compare(ref, got):
    """-> (max_excess_over_gate, max_abs, bit_identical, note)"""
    if ref["raw"] is None or got["raw"] is None:
        return math.inf, math.inf, False, "missing raw"
    if len(ref["raw"]) != len(got["raw"]):
        return math.inf, math.inf, False, f"plot count {len(ref['raw'])} vs {len(got['raw'])}"
    worst, mx, ident = 0.0, 0.0, True
    for a, b in zip(ref["raw"], got["raw"]):
        if a["names"] != b["names"] or a["data"].shape != b["data"].shape:
            return math.inf, math.inf, False, "plot shape/names differ"
        ident = ident and a["data"].tobytes() == b["data"].tobytes()
        d = np.abs(a["data"] - b["data"])
        gate = GATE_ABS + GATE_REL * np.abs(a["data"])
        # axis column (time / sweep value) must agree to 1e-12 relative
        if a["data"].shape[0] > 1:
            ax = np.abs(a["data"][:, 0] - b["data"][:, 0])
            if np.any(ax > 1e-12 * np.maximum(1e-12, np.abs(a["data"][:, 0]))):
                return math.inf, math.inf, False, "axis differs"
        mx = max(mx, float(d.max()))
        worst = max(worst, float((d / gate).max()))
    return worst, mx, ident, ""


def equivalence(rep, args, tmp):
    decks = {s: os.path.join(args.decks, PREFIX + s + ".sp") for s in FIXTURES}
    decks.update(gen_decks(tmp))
    table = {}
    for name, deck in decks.items():
        ref = run_backend(args.bin, deck, "cpu", tmp, "ref")
        if name == "gen_level2_divider" and ref["rc"] != 0 and "LEVEL=2 is not implemented" in ref["err"]:
            all_refuse = all(
                run_backend(args.bin, deck, b, tmp, f"refuse_{b}")["rc"] != 0
                for b in BACKENDS[1:]
            )
            rep.add(
                f"equiv/{name}/refused",
                "GAP_CLOSED" if all_refuse else "FAIL",
                "LEVEL=2 refused across all backends (was silent-wrong-model-level / backend-df64-mos-model-mismatch)",
            )
            continue
        if ref["rc"] != 0 or ref["raw"] is None:
            rep.add(f"equiv/{name}/cpu_runs", "FAIL", f"rc={ref['rc']} {ref['err'][-150:]}")
            continue
        again = run_backend(args.bin, deck, "cpu", tmp, "again")
        thr = [run_backend(args.bin, deck, "cpu", tmp, f"thr{t}", env=dict(os.environ, RAYON_NUM_THREADS=t)) for t in ("1", "8")]
        det = all(c["raw"] is not None and compare(ref, c)[2] for c in [again] + thr)
        rep.add(f"equiv/{name}/cpu_deterministic", "PASS" if det else "FAIL", "bit-identical across repeat and RAYON_NUM_THREADS=1/8" if det else "cpu raw output differs between identical runs")
        truth, truth_err = None, ""
        if name not in NO_TRUTH and name not in TRAN:
            try:
                truth = ngspice_op(open(deck).read(), tmp, tag="truth", ngspice=args.ngspice)  # black-box tightly converged reference
            except (RuntimeError, OSError, ValueError) as ex:
                truth = None
                truth_err = str(ex)[:80]
        cpu_vec = {n: float(v) for n, v in zip(ref["raw"][-1]["names"], ref["raw"][-1]["data"][-1])} if truth else None
        for b in BACKENDS[1:]:
            got = run_backend(args.bin, deck, b, tmp, "got")
            if got["rc"] != 0:
                rep.add(f"equiv/{name}/{b}", "FAIL", f"rc={got['rc']} {got['err'][-150:]}")
                continue
            ok_stats = got["stats"] and got["stats"].get("status") == "ok" and got["stats"].get("points") == ref["stats"].get("points")
            excess, mx, ident, note = compare(ref, got)
            table[(name, b)] = (mx, ident, got["label"])
            expected = EXPECTED_GAPS.get((name, b))
            info = f"max|diff vs cpu|={mx:.2e} (gate use {excess:.1e}) bit_identical={ident} label={got['label']} {note}"
            if excess <= 1.0 and ok_stats:
                if expected:
                    rep.add(f"equiv/{name}/{b}", "GAP_CLOSED", info + f" (was {expected})")
                else:
                    rep.add(f"equiv/{name}/{b}", "PASS", info)
                continue
            # mismatch: decide who is wrong before calling it anything
            if truth:
                got_vec = {n: float(v) for n, v in zip(got["raw"][-1]["names"], got["raw"][-1]["data"][-1])}
                e_cpu, e_got = truth_agrees(cpu_vec, truth), truth_agrees(got_vec, truth)
                who = f"vs ngspice(tight): cpu excess {e_cpu:.1e}, {b} excess {e_got:.1e} (gate 1)"
                if e_cpu > 1.0:
                    rep.add(f"equiv/{name}/{b}", "FAIL", info + f"; cpu itself disagrees {who}")
                    continue
            else:
                who = "no independent arbiter for this deck" if name in NO_TRUTH else ("transient: attributed via the identical device cards of backend_nand_chain_op" if name in TRAN else f"UNVERIFIED arbiter ({truth_err})")
                if name not in NO_TRUTH and name not in TRAN:
                    rep.add(f"equiv/{name}/{b}", "UNVERIFIED", info + f"; {who}")
                    continue
            if expected:
                rep.add(f"equiv/{name}/{b}", "KNOWN_GAP_CONFIRMED", f"[{expected}] {info}; {who}")
            else:
                rep.add(f"equiv/{name}/{b}", "FAIL", info + f"; {who}")
    return table


def honesty(rep, args, tmp, table):
    sysname = platform.system()
    small = [k for k in table if k[0] in FIXTURES]
    # --- (1) metal on a non-Apple host
    lab = {k[1]: v[2] for k, v in table.items()}
    if sysname != "Darwin" and lab.get("metal"):
        rep.add("gap/backend-label-not-executor/metal", "KNOWN_GAP_CONFIRMED", f"banner '{lab['metal']}' on {sysname}: no Metal device exists")
    elif sysname == "Darwin":
        rep.add("gap/backend-label-not-executor/metal", "UNVERIFIED", "Darwin host: needs a GPU-activity probe")
    # --- (2) no CUDA context for cuda-* backends (nvidia-smi poll with torch positive control)
    smi = shutil.which("nvidia-smi")
    if not smi:
        rep.add("gap/backend-label-not-executor/cuda", "UNVERIFIED", "nvidia-smi not available")
    else:
        long_deck = tmp.f("long_rc.sp")
        open(long_deck, "w").write("cap_cli long rc transient\nV1 in 0 SIN(0 1 1e6)\nR1 in out 1k\nC1 out 0 1n\n.tran 1n 1m\n.end\n")

        def listed(cmd):
            p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            seen, polls = False, 0
            while p.poll() is None:
                o = subprocess.run([smi, "--query-compute-apps=pid", "--format=csv,noheader"], capture_output=True, text=True).stdout
                polls += 1
                seen = seen or str(p.pid) in o.split()
            return seen, polls

        control = None
        if args.gpu_control:
            try:
                control, _ = listed([sys.executable, "-c", "import torch,time; torch.zeros(1,device='cuda'); time.sleep(2.5)"])
            except Exception:
                control = None
        for b in ("cuda-tile", "cuda-simt"):
            seen, polls = listed([args.bin, "run", long_deck, "--backend", b])
            if polls < 3:
                rep.add(f"gap/backend-label-not-executor/{b}", "UNVERIFIED", f"run too short ({polls} polls)")
            elif seen:
                rep.add(f"gap/backend-label-not-executor/{b}", "GAP_CLOSED", "a CUDA context is now created: re-check numerical equivalence with a real device")
            else:
                ctl = {True: "positive control (torch context) was listed", False: "WARNING: positive control NOT listed", None: "no positive control run (--gpu-control)"}[control]
                st = "KNOWN_GAP_CONFIRMED" if control is not False else "UNVERIFIED"
                rep.add(f"gap/backend-label-not-executor/{b}", st, f"banner '{lab.get(b)}' but the process never owned a CUDA context ({polls} nvidia-smi polls); {ctl}")
    # --- (3) bit-identical to cpu on every small deck: the 'device' arithmetic is the CPU code path
    for b in ("cuda-tile", "cuda-simt"):
        idn = [table[k][1] for k in small if k[1] == b]
        rep.add(f"gap/backend-label-not-executor/{b}_bit_identical_to_cpu", "INFO", f"{sum(idn)}/{len(idn)} small decks bit-identical to cpu (a real device would differ in the last bits)")
    # --- (3b) the wasm result depends on the device count (double-float path from 1024 nonlinear devices on)
    small_w, big_w = table.get(("gen_follower_8", "wasm")), table.get(("gen_follower_1100", "wasm"))
    if small_w and big_w:
        if small_w[1] and not big_w[1]:
            rep.add("gap/backend-df64-mos-model-mismatch/wasm_depends_on_device_count", "KNOWN_GAP_CONFIRMED", f"identical follower cell: 8 cells bit-identical to cpu, 1100 cells off by {big_w[0]:.2e} V")
        elif small_w[1] and big_w[1]:
            rep.add("gap/backend-df64-mos-model-mismatch/wasm_depends_on_device_count", "GAP_CLOSED", "1100-cell result now bit-identical to cpu")
    # --- (4) stats lacks executor identity
    d = os.path.join(args.decks, PREFIX + "backend_nand_chain_op.sp")
    r = run_backend(args.bin, d, "wasm", tmp, "st")
    keys = sorted(r["stats"] or {})
    if not any(re.search(r"backend|executor|device", k) for k in keys):
        rep.add("gap/stats-lacks-executor", "KNOWN_GAP_CONFIRMED", f"stats keys {keys}: no backend/executor field")
    else:
        rep.add("gap/stats-lacks-executor", "GAP_CLOSED", f"stats keys {keys}")
    # --- (5) strictness of option values
    rc, out, err, _ = run([args.bin, "run", d, "--backend", "gpu"])
    lenient = rc == 0 and "cpu-pulp-simd" in out
    rep.add("gap/cli-backend-typo-silent-cpu", "KNOWN_GAP_CONFIRMED" if lenient else ("GAP_CLOSED" if rc not in (0, 101, None) else "FAIL"), f"`--backend gpu` rc={rc}, banner={'cpu-pulp-simd' if 'cpu-pulp-simd' in out else err[:60]!r}")
    rc, out, err, _ = run([args.bin, "codegen", "--model", "bogus", "--target", "nonsense"])
    lenient = rc == 0 and "mosfet_level1" in out
    rep.add("gap/cli-codegen-option-silent", "KNOWN_GAP_CONFIRMED" if lenient else ("GAP_CLOSED" if rc not in (0, 101, None) else "FAIL"), f"`codegen --model bogus --target nonsense` rc={rc}, emitted {'the default mosfet-l1/msl kernel' if lenient else err[:60]!r}")
    # accepted spellings keep working (guards against over-correction when the gap closes)
    for b in BACKENDS:
        rc, out, _, _ = run([args.bin, "run", d, "--backend", b])
        rep.add(f"cli/backend_{b}_accepted", "PASS" if rc == 0 else "FAIL", f"rc={rc}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default=DEFAULT_BIN)
    ap.add_argument("--ngspice", default=DEFAULT_NGSPICE)
    ap.add_argument("--decks", default=DECK_DIR, help="directory with the cap_cli_* fixture decks (default: benchmarks/cases/capability)")
    ap.add_argument("--cpus", default=None)
    ap.add_argument("--json", default=None)
    ap.add_argument("--gpu-control", action="store_true", help="run a torch CUDA process as positive control for the nvidia-smi probe")
    ap.add_argument("--strict", action="store_true")
    args = ap.parse_args()
    pin_cpus(args.cpus)
    if not os.access(args.bin, os.X_OK):
        print("unverified: rustspice binary not available")
        return 2
    rep, tmp = Report(), Tmp()
    try:
        table = equivalence(rep, args, tmp)
        honesty(rep, args, tmp, table)
    finally:
        tmp.close()
    gaps = sum(1 for r in rep.rows if r["status"] == "KNOWN_GAP_CONFIRMED")
    print(f"\nsummary: {len(rep.rows)} checks, {rep.bad} unexpected failures, {gaps} known gaps confirmed")
    if args.json:
        json.dump({"script": "check_backend_equiv", "bad": rep.bad, "rows": rep.rows}, open(args.json, "w"), indent=1)
    return 1 if rep.bad or (args.strict and gaps) else 0


if __name__ == "__main__":
    sys.exit(main())
