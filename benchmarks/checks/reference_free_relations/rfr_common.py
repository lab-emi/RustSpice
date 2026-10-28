"""Shared helpers for the reference-free relation checks (acc_rel_* decks).

Rule 1: ngspice is used only as a black-box executable (control). Everything else here is independent of both
simulators: a mini deck parser + dense float64 MNA solver (numpy, one longdouble refinement step).
Rule 9: every child runs under a wall-clock `timeout` (and `taskset` when RFR_CPUS is set); sizes are small
(<= ~2500 unknowns, a few MB).
"""
import json
import os
import re
import struct
import subprocess
import sys
import tempfile
import time

import numpy as np

# <repo>/modules/RustSpice/benchmarks/checks/reference_free_relations/rfr_common.py -> parents[3] = <repo>/modules/RustSpice
ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
CASES = os.path.join(ROOT, "benchmarks", "cases")
RS_BIN = os.environ.get("RUSTSPICE", os.path.join(ROOT, "target", "release", "rustspice"))
NG_BIN = os.environ.get("NGSPICE", "ngspice")
CPUS = os.environ.get("RFR_CPUS", "")  # taskset list for every child, e.g. 16-31; default: no pinning
TIGHT = "reltol=1e-9 vntol=1e-12 abstol=1e-15"


def default_work(name):
    """Scratch directory (never the repository): <tmpfs or system temp>/rfr/<name>."""
    return os.path.join("/dev/shm" if os.path.isdir("/dev/shm") else tempfile.gettempdir(), "rfr", name)


def _pre(timeout_s):
    return (["taskset", "-c", CPUS] if CPUS else []) + ["timeout", str(timeout_s)]


# --------------------------------------------------------------------------------------------- raw files
def parse_raw(path):
    """Parse an ngspice-compatible binary raw file into a list of plots.

    Each plot is {"name", "vars": [names], "data": ndarray (npoints, nvars)}.  Variable names are lower case.
    """
    blob = open(path, "rb").read()
    plots = []
    pos = 0
    while pos < len(blob):
        j = blob.find(b"Binary:\n", pos)
        if j < 0:
            break
        head = blob[pos:j].decode("latin1")
        nvar = int(re.search(r"No\. Variables:\s*(\d+)", head).group(1))
        npts = int(re.search(r"No\. Points:\s*(\d+)", head).group(1))
        name = re.search(r"Plotname:\s*(.*)", head).group(1).strip()
        vars_ = []
        vsec = re.split(r"\nVariables:", head, maxsplit=1)[1]
        for ln in vsec.strip().splitlines():
            parts = ln.split()
            if len(parts) >= 2:
                vars_.append(parts[1].lower())
        assert len(vars_) == nvar, (len(vars_), nvar)
        start = j + len(b"Binary:\n")
        nbytes = nvar * npts * 8
        arr = np.frombuffer(blob[start : start + nbytes], dtype="<f8").reshape(npts, nvar)
        plots.append({"name": name, "vars": vars_, "data": arr.copy()})
        pos = start + nbytes
    return plots


def col(plot, name):
    return plot["data"][:, plot["vars"].index(name.lower())]


# --------------------------------------------------------------------------------------------- runners
def run_rs(deck, workdir, tag="rs", extra_env=None, timeout=60):
    raw = os.path.join(workdir, f"{tag}.raw")
    stats = os.path.join(workdir, f"{tag}.json")
    env = dict(os.environ)
    if extra_env:
        env.update(extra_env)
    t0 = time.time()
    p = subprocess.run(
        _pre(timeout) + [RS_BIN, "run", deck, "--raw", raw, "--stats", stats],
        capture_output=True,
        text=True,
        env=env,
    )
    wall = time.time() - t0
    if p.returncode != 0 or not os.path.exists(raw):
        raise RuntimeError(f"rustspice failed rc={p.returncode}: {p.stderr[-400:]}")
    st = json.load(open(stats))
    return parse_raw(raw), st, wall


def with_options(deck_text, options):
    """Insert `.options ...` before the final .end (ngspice only; RustSpice ignores .options)."""
    lines = deck_text.rstrip("\n").split("\n")
    idx = max(i for i, l in enumerate(lines) if l.strip().lower() == ".end")
    lines.insert(idx, ".options " + options)
    return "\n".join(lines) + "\n"


def run_ng(deck, workdir, tag="ng", options=TIGHT, timeout=120, klu=False):
    text = open(deck).read()
    opts = options or ""
    if klu:
        opts += " klu"
    if opts.strip():
        text = with_options(text, opts)
    d2 = os.path.join(workdir, f"{tag}.sp")
    open(d2, "w").write(text)
    raw = os.path.join(workdir, f"{tag}.raw")
    t0 = time.time()
    p = subprocess.run(
        _pre(timeout) + [NG_BIN, "-b", "-r", raw, d2], capture_output=True, text=True, cwd=workdir
    )
    wall = time.time() - t0
    if not os.path.exists(raw):
        raise RuntimeError(f"ngspice failed rc={p.returncode}: {(p.stdout + p.stderr)[-400:]}")
    return parse_raw(raw), p.stdout + p.stderr, wall


def last_plot(plots, prefix):
    sel = [p for p in plots if p["name"].lower().startswith(prefix.lower())]
    return sel[-1]


# --------------------------------------------------------------------------------------------- numbers
_SUF = {"t": 1e12, "g": 1e9, "meg": 1e6, "k": 1e3, "m": 1e-3, "u": 1e-6, "n": 1e-9, "p": 1e-12, "f": 1e-15, "a": 1e-18}


def num(tok):
    tok = tok.strip().lower()
    m = re.fullmatch(r"([+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)(meg|[tgkmunpfa])?[a-z]*", tok)
    if not m:
        raise ValueError(tok)
    v = float(m.group(1))
    if m.group(2):
        v *= _SUF[m.group(2)]
    return v


# --------------------------------------------------------------------------------------------- independent oracle
def parse_linear_deck(text):
    """Flat deck with R, V, I, G cards only (no params, no continuation). Returns element lists."""
    els = []
    for ln in text.splitlines()[1:]:
        s = ln.strip()
        if not s or s[0] in "*.+":
            continue
        t = s.split()
        k = t[0][0].lower()
        if k == "r":
            els.append(("R", t[0].lower(), t[1].lower(), t[2].lower(), num(t[3])))
        elif k == "v":
            v = t[3] if t[3].lower() != "dc" else t[4]
            els.append(("V", t[0].lower(), t[1].lower(), t[2].lower(), num(v)))
        elif k == "i":
            v = t[3] if t[3].lower() != "dc" else t[4]
            els.append(("I", t[0].lower(), t[1].lower(), t[2].lower(), num(v)))
        elif k == "g":
            els.append(("G", t[0].lower(), t[1].lower(), t[2].lower(), (t[3].lower(), t[4].lower(), num(t[5]))))
        else:
            raise ValueError(f"unsupported card for the oracle: {s}")
    return els


def mna_solve(text):
    """Dense float64 MNA (+ one longdouble refinement step). Returns {node: v, 'i(vname)': i}."""
    els = parse_linear_deck(text)
    nodes = {"0": 0, "gnd": 0}
    def nid(n):
        if n not in nodes:
            nodes[n] = len(set(nodes.values()))
        return nodes[n]
    for e in els:
        nid(e[2]); nid(e[3])
        if e[0] == "G":
            nid(e[4][0]); nid(e[4][1])
    nn = len(set(nodes.values())) - 1
    vs = [e for e in els if e[0] == "V"]
    dim = nn + len(vs)
    A = np.zeros((dim, dim)); b = np.zeros(dim)
    def ix(n):
        return nodes[n] - 1
    for e in els:
        kind, _, a, c, val = e
        ia, ic = ix(a), ix(c)
        if kind == "R":
            g = 1.0 / val
            for (p, q, s) in ((ia, ia, g), (ic, ic, g), (ia, ic, -g), (ic, ia, -g)):
                if p >= 0 and q >= 0:
                    A[p, q] += s
        elif kind == "I":  # current flows a -> c through the source
            if ia >= 0: b[ia] -= val
            if ic >= 0: b[ic] += val
        elif kind == "G":  # i(a->c) = gm * (v(cp) - v(cn))
            cp, cn, gm = val
            ip, im = ix(cp), ix(cn)
            for (r, s1) in ((ia, 1.0), (ic, -1.0)):
                if r < 0: continue
                if ip >= 0: A[r, ip] += s1 * gm
                if im >= 0: A[r, im] -= s1 * gm
    for k, e in enumerate(vs):
        _, _, a, c, val = e
        ia, ic = ix(a), ix(c)
        row = nn + k
        if ia >= 0: A[ia, row] += 1; A[row, ia] += 1
        if ic >= 0: A[ic, row] -= 1; A[row, ic] -= 1
        b[row] = val
    x = np.linalg.solve(A, b)
    Al = A.astype(np.longdouble); bl = b.astype(np.longdouble)
    for _ in range(2):
        r = bl - Al @ x.astype(np.longdouble)
        x = x + np.linalg.solve(A, r.astype(np.float64))
    out = {n: (x[i - 1] if i > 0 else 0.0) for n, i in nodes.items()}
    for k, e in enumerate(vs):
        out["i(" + e[1] + ")"] = x[nn + k]
    return out, dict(nodes=nodes, A=A, b=b)


# --------------------------------------------------------------------------------------------- mesh generator
def lcg_rng(seed):
    """xorshift64* (deterministic, no numpy RNG version dependence)."""
    s = (seed * 0x9E3779B97F4A7C15 + 0x1234567) & 0xFFFFFFFFFFFFFFFF
    s = s or 1
    while True:
        s ^= s >> 12
        s ^= (s << 25) & 0xFFFFFFFFFFFFFFFF
        s ^= s >> 27
        yield ((s * 0x2545F4914F6CDD1D) & 0xFFFFFFFFFFFFFFFF) / 2.0**64


def mesh_links(n, seed, rlo, rhi):
    """Natural-order list of (name, node_a, node_b, R) for an n x n mesh; R log-uniform in [rlo, rhi]."""
    rng = lcg_rng(seed)
    out = []
    for i in range(n):
        for j in range(n):
            if j + 1 < n:
                r = rlo * (rhi / rlo) ** next(rng)
                out.append((f"rh{i}_{j}", f"{i}_{j}", f"{i}_{j + 1}", float(f"{r:.5g}")))
            if i + 1 < n:
                r = rlo * (rhi / rlo) ** next(rng)
                out.append((f"rv{i}_{j}", f"{i}_{j}", f"{i + 1}_{j}", float(f"{r:.5g}")))
    return out


def fmt(x):
    return f"{x:.6g}"


def shuffled(seq, seed):
    rng = lcg_rng(seed)
    a = list(seq)
    for i in range(len(a) - 1, 0, -1):
        j = int(next(rng) * (i + 1))
        a[i], a[j] = a[j], a[i]
    return a
