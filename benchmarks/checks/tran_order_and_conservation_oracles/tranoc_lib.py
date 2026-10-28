"""Shared helpers for the tran_order_and_conservation_oracles decks and checks.

Everything here treats both simulators as black-box executables (AGENTS.md Rule 1): decks go in, raw files and
console output come out.  The oracles live in the check_*.py scripts and use only closed forms or scipy.
"""
import os
import pathlib
import re
import subprocess
import sys
import tempfile

import numpy as np

# <repo>/modules/RustSpice/benchmarks/checks/tran_order_and_conservation_oracles/tranoc_lib.py -> parents[3] = <repo>/modules/RustSpice
ROOT = pathlib.Path(__file__).resolve().parents[3]
CASES = ROOT / "benchmarks" / "cases"
RS = os.environ.get("RUSTSPICE", str(ROOT / "target" / "release" / "rustspice"))
NG = os.environ.get("NGSPICE", "ngspice")
# scratch directory, never the repository (override with TRANOC_WORK)
WORK = os.environ.get("TRANOC_WORK") or os.path.join("/dev/shm" if os.path.isdir("/dev/shm") else tempfile.gettempdir(), "tranoc", "work")
os.makedirs(WORK, exist_ok=True)

# Optional CPU pinning of every child, e.g. TRANOC_CPUS=16-31 (default: no pinning).
PIN = ["taskset", "-c", os.environ["TRANOC_CPUS"]] if os.environ.get("TRANOC_CPUS") else []


def read_raw(path):
    """ngspice-style binary raw file -> list of plots {name, vars, data[nvars, npts]}."""
    b = open(path, "rb").read()
    plots = []
    pos = 0
    while pos < len(b):
        i = b.find(b"Binary:\n", pos)
        if i < 0:
            break
        hdr = b[pos:i].decode("latin1")
        nv = int(re.search(r"No. Variables:\s*(\d+)", hdr).group(1))
        npts = int(re.search(r"No. Points:\s*(\d+)", hdr).group(1))
        name = re.search(r"Plotname:\s*(.*)", hdr).group(1).strip()
        vs = re.search(r"Variables:\n(.*)", hdr, re.S).group(1).strip().splitlines()
        names = [l.split()[1].lower() for l in vs]
        start = i + len(b"Binary:\n")
        n = nv * npts * 8
        arr = np.frombuffer(b[start : start + n], dtype="<f8").reshape(npts, nv).T
        plots.append(dict(name=name, vars=names, data=arr))
        pos = start + n
    return plots


def _tran(plots):
    for p in plots:
        if p["name"].lower().startswith("transient"):
            return {v: p["data"][k].copy() for k, v in enumerate(p["vars"])}
    raise RuntimeError("no transient plot")


def run_rs(deck, tag, timeout=60):
    """Run the frozen RustSpice binary; returns dict of vectors (including 'time') or raises."""
    d = os.path.join(WORK, tag)
    os.makedirs(d, exist_ok=True)
    sp = os.path.join(d, "rs.sp")
    open(sp, "w").write(deck)
    raw = os.path.join(d, "rs.raw")
    if os.path.exists(raw):
        os.remove(raw)
    r = subprocess.run(
        PIN + [RS, "run", sp, "--raw", raw, "--stats", os.path.join(d, "rs.json")],
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    if r.returncode != 0 or not os.path.exists(raw):
        raise RuntimeError(f"rustspice failed rc={r.returncode}: {r.stderr[-300:]}{r.stdout[-300:]}")
    return _tran(read_raw(raw))


def with_options(deck, options=None, tmax=None):
    """Insert .options before .end and (optionally) a tmax on the .tran card."""
    out = []
    for l in deck.splitlines():
        s = l.strip().lower()
        if s.startswith(".tran") and tmax is not None:
            t = l.split()
            # .tran tstep tstop [tstart [tmax]] [uic]
            uic = [x for x in t[3:] if x.lower() == "uic"]
            rest = [x for x in t[3:] if x.lower() != "uic"]
            tstart = rest[0] if rest else "0"
            l = " ".join(t[:3] + [tstart, repr(float(tmax))] + uic)
        if s == ".end" and options:
            out.append(".options " + options)
        out.append(l)
    return "\n".join(out) + "\n"


def run_ng(deck, tag, options=None, tmax=None, timeout=90):
    d = os.path.join(WORK, tag)
    os.makedirs(d, exist_ok=True)
    sp = os.path.join(d, "ng.sp")
    open(sp, "w").write(with_options(deck, options, tmax))
    raw = os.path.join(d, "ng.raw")
    if os.path.exists(raw):
        os.remove(raw)
    r = subprocess.run(
        PIN + [NG, "-b", "-r", raw, sp], capture_output=True, text=True, timeout=timeout, cwd=d
    )
    if not os.path.exists(raw):
        raise RuntimeError(f"ngspice failed rc={r.returncode}: {r.stdout[-300:]}")
    txt = r.stdout + r.stderr
    bad = [p for p in ("singular matrix", "no convergence", "Timestep too small", "Too many iterations") if p in txt]
    if bad:
        raise RuntimeError(f"ngspice reported {bad}")
    return _tran(read_raw(raw))


def with_tstep(deck, tstep):
    out = []
    for l in deck.splitlines():
        if l.strip().lower().startswith(".tran"):
            t = l.split()
            t[1] = repr(float(tstep))
            l = " ".join(t)
        out.append(l)
    return "\n".join(out) + "\n"


def loglog_slopes(h, e):
    h = np.asarray(h, float)
    e = np.asarray(e, float)
    return np.diff(np.log(e)) / np.diff(np.log(h))


def fmt(x):
    return "null" if x is None or (isinstance(x, float) and not np.isfinite(x)) else f"{x:.3e}"


def spice_num(tok):
    """SPICE number with engineering suffix -> float (meg before m)."""
    t = tok.strip().lower()
    m = re.match(r"^([+-]?[0-9]*\.?[0-9]+(?:e[+-]?[0-9]+)?)([a-z]*)$", t)
    if not m:
        raise ValueError(tok)
    mult = {"": 1, "f": 1e-15, "p": 1e-12, "n": 1e-9, "u": 1e-6, "m": 1e-3, "k": 1e3, "meg": 1e6, "g": 1e9, "t": 1e12}
    suf = m.group(2)
    for key in ("meg",):
        if suf.startswith(key):
            return float(m.group(1)) * mult[key]
    return float(m.group(1)) * mult.get(suf[:1], 1)


def deck_meta(path):
    """(deck text, {key: value} of the bench.* lines)."""
    text = open(path).read()
    meta = {}
    for l in text.splitlines()[1:]:
        m = re.match(r"^\*\s*bench\.([a-z.]+):\s*(.*)$", l.strip())
        if m:
            meta[m.group(1)] = m.group(2).strip()
    return text, meta


def series_steps(meta):
    return [spice_num(x) for x in meta["series.tstep"].split()]


def tran_card(text):
    for l in text.splitlines():
        if l.strip().lower().startswith(".tran"):
            t = l.split()
            return spice_num(t[1]), spice_num(t[2])
    raise ValueError("no .tran")


def element_value(text, name, idx=-1):
    """Value token of element `name` (last field by default)."""
    for l in text.splitlines():
        t = l.split()
        if t and t[0].lower() == name.lower():
            return spice_num(t[idx])
    raise KeyError(name)


def table(rows, header):
    w = [max(len(str(r[i])) for r in [header] + rows) for i in range(len(header))]
    out = ["  ".join(str(h).ljust(w[i]) for i, h in enumerate(header))]
    for r in rows:
        out.append("  ".join(str(c).ljust(w[i]) for i, c in enumerate(r)))
    return "\n".join(out)
