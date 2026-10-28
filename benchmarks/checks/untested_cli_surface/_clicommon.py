"""Shared helpers for the cap_cli_* check scripts.

Dependencies: python3 stdlib, numpy, mpmath.  ngspice is used ONLY as a black-box executable (AGENTS.md Rule 1):
its observable output (rawfile) is read, nothing else.  The independent oracle is a 50-digit mpmath Newton solve of
the textbook MNA equations of the deck (R, V, I, diode with SPICE3 reverse-bias branch, level-1 MOSFET with its bulk junctions) with
implicit-function-theorem parameter sensitivities; it shares no code and no numerical method with RustSpice or ngspice.
"""
from __future__ import annotations

import math
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field

import mpmath as mp
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
# <repo>/modules/RustSpice/benchmarks/checks/untested_cli_surface/_clicommon.py -> three levels up = <repo>/modules/RustSpice
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
DECK_DIR = os.path.join(ROOT, "benchmarks", "cases", "capability")  # the cap_cli_* fixture decks
DEFAULT_BIN = os.environ.get("RUSTSPICE", os.path.join(ROOT, "target", "release", "rustspice"))
DEFAULT_NGSPICE = os.environ.get("NGSPICE", "ngspice")
TIGHT = "reltol=1e-9 vntol=1e-12 abstol=1e-15"
# value both RustSpice (netlist path) and ngspice-47 use for kT/q at 300.15 K; calibrated black-box by the
# agreement of the oracle with the tightly converged ngspice OP (see check_adjoint_fd.py --calibrate).
VT_300K15 = 0.025864917007155805
GMIN = 1e-12

mp.mp.dps = 50


# --------------------------------------------------------------------------------------------- process helpers
def pin_cpus(spec: str | None) -> None:
    if spec:
        lo, _, hi = spec.partition("-")
        cpus = set(range(int(lo), int(hi or lo) + 1))
        os.sched_setaffinity(0, cpus)


def run(cmd, timeout=60, cwd=None, env=None):
    t0 = time.monotonic()
    try:
        p = subprocess.run(cmd, capture_output=True, timeout=timeout, cwd=cwd, env=env)
        return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace"), time.monotonic() - t0
    except subprocess.TimeoutExpired:
        return None, "", "TIMEOUT", time.monotonic() - t0


class Tmp:
    """Scratch directory under /dev/shm, else the system temp dir (never the repository)."""

    def __init__(self, prefix="clichk_"):
        base = "/dev/shm" if os.path.isdir("/dev/shm") else None
        self.path = tempfile.mkdtemp(prefix=prefix, dir=base)

    def f(self, name):
        return os.path.join(self.path, name)

    def close(self):
        shutil.rmtree(self.path, ignore_errors=True)


# ----------------------------------------------------------------------------------------------- rawfile reader
def read_raw(path):
    """ngspice binary rawfile -> list of plots {plotname, names, kinds, data[npoints][nvars]}; real-valued only."""
    with open(path, "rb") as fh:
        blob = fh.read()
    plots = []
    pos = 0
    while pos < len(blob):
        hdr = {}
        names, kinds = [], []
        while True:
            nl = blob.index(b"\n", pos)
            line = blob[pos:nl].decode("latin1")
            pos = nl + 1
            if line.startswith("Variables:"):
                nv = int(hdr["No. Variables"])
                for _ in range(nv):
                    nl = blob.index(b"\n", pos)
                    parts = blob[pos:nl].decode("latin1").split()
                    pos = nl + 1
                    names.append(parts[1].lower())
                    kinds.append(parts[2])
                continue
            if line.startswith("Binary:"):
                break
            k, _, v = line.partition(":")
            hdr[k.strip()] = v.strip()
        nv, npts = int(hdr["No. Variables"]), int(hdr["No. Points"])
        if "complex" in hdr.get("Flags", ""):
            raise ValueError("complex rawfile not supported")
        n = nv * npts
        data = np.frombuffer(blob, dtype="<f8", count=n, offset=pos).reshape(npts, nv)
        pos += 8 * n
        plots.append({"plotname": hdr.get("Plotname", ""), "names": names, "kinds": kinds, "data": data})
        if pos >= len(blob):
            break
    return plots


def op_vector(plots):
    """Last plot with one point -> {name: value}."""
    for p in reversed(plots):
        if p["data"].shape[0] == 1:
            return {n: float(v) for n, v in zip(p["names"], p["data"][0])}
    raise ValueError("no single-point plot")


# ------------------------------------------------------------------------------------------------ tool runners
def with_options(text: str, options: str) -> str:
    """Insert a `.options` card in front of the last `.end` (or append)."""
    lines = text.rstrip("\n").split("\n")
    out, done = [], False
    for ln in reversed(lines):
        if not done and ln.strip().lower() == ".end":
            out.append(ln)
            out.append(f".options {options}")
            done = True
        else:
            out.append(ln)
    if not done:
        out = [f".options {options}"] + out
    return "\n".join(reversed(out)) + "\n"


def ngspice_op(text, tmp: Tmp, tag="ng", options=TIGHT, ngspice=DEFAULT_NGSPICE, timeout=30):
    deck, raw = tmp.f(f"{tag}.sp"), tmp.f(f"{tag}.raw")
    with open(deck, "w") as fh:
        fh.write(with_options(text, options) if options else text)
    if os.path.exists(raw):
        os.unlink(raw)
    rc, out, err, _ = run([ngspice, "-b", "-r", raw, deck], timeout=timeout)
    if rc != 0 or not os.path.exists(raw):
        raise RuntimeError(f"ngspice failed rc={rc}: {(out + err)[-400:]}")
    return op_vector(read_raw(raw))


def rustspice_op(text, tmp: Tmp, tag="rs", binary=DEFAULT_BIN, backend="cpu", timeout=30):
    deck, raw = tmp.f(f"{tag}.sp"), tmp.f(f"{tag}.raw")
    with open(deck, "w") as fh:
        fh.write(text)
    if os.path.exists(raw):
        os.unlink(raw)
    rc, out, err, _ = run([binary, "run", deck, "--backend", backend, "--raw", raw], timeout=timeout)
    if rc != 0 or not os.path.exists(raw):
        raise RuntimeError(f"rustspice failed rc={rc}: {(out + err)[-400:]}")
    return op_vector(read_raw(raw))


def parse_adjoint_stdout(out: str):
    """-> (n_reported, {'R1.r': value, ...}) from the `adjoint` CLI text."""
    m = re.search(r"Computed (\d+) parameter sensitivities", out)
    n = int(m.group(1)) if m else None
    grads = {}
    for mm in re.finditer(r"dV\((\S+?)\)/d\((\S+?)\) = (\S+)", out):
        grads[mm.group(2).lower()] = float(mm.group(3))
    return n, grads


# ------------------------------------------------------------------------------------------- netlist (subset)
_SUFFIX = {"t": 1e12, "g": 1e9, "meg": 1e6, "k": 1e3, "m": 1e-3, "u": 1e-6, "n": 1e-9, "p": 1e-12, "f": 1e-15}


def parse_value(tok: str) -> mp.mpf:
    t = tok.strip().lower()
    m = re.fullmatch(r"([+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)(meg|[tgkmunpf])?[a-z]*", t)
    if not m:
        raise ValueError(f"bad value {tok!r}")
    v = mp.mpf(m.group(1))
    if m.group(2):
        v *= mp.mpf(repr(_SUFFIX[m.group(2)]))
    return v


@dataclass
class Elem:
    name: str
    kind: str  # r v i d m
    nodes: list
    value: mp.mpf | None = None
    model: str | None = None
    params: dict = field(default_factory=dict)


@dataclass
class Circuit:
    title: str
    elems: list
    models: dict  # name -> {'type': 'd'|'nmos'|'pmos', params...}
    analysis: list


def _kv(tokens):
    d = {}
    for t in tokens:
        if "=" in t:
            k, _, v = t.partition("=")
            d[k.strip().lower()] = v.strip()
    return d


def parse_deck(text: str) -> Circuit:
    lines = text.splitlines()
    title = lines[0] if lines else ""
    # join continuation lines
    logical = []
    for ln in lines[1:]:
        if ln.startswith("+") and logical:
            logical[-1] += " " + ln[1:]
        else:
            logical.append(ln)
    elems, models, analysis = [], {}, []
    for ln in logical:
        s = ln.strip()
        if not s or s.startswith("*"):
            continue
        s = s.replace("(", " ").replace(")", " ").replace(",", " ")
        tok = s.split()
        c = tok[0].lower()
        if c.startswith("."):
            if c == ".model":
                kv = _kv(tok[3:])
                typ = tok[2].lower()
                models[tok[1].lower()] = {"type": typ, **kv}
            elif c in (".op", ".dc", ".tran"):
                analysis.append(tok)
            continue
        k = c[0]
        if k == "r":
            elems.append(Elem(tok[0], "r", tok[1:3], parse_value(tok[3])))
        elif k in "vi":
            val = None
            rest = [t.lower() for t in tok[3:]]
            if rest and rest[0] == "dc":
                val = parse_value(tok[4])
            elif rest:
                val = parse_value(tok[3])
            elems.append(Elem(tok[0], k, tok[1:3], val))
        elif k == "c":
            elems.append(Elem(tok[0], "c", tok[1:3], parse_value(tok[3])))  # open circuit at DC
        elif k == "l":
            elems.append(Elem(tok[0], "l", tok[1:3], parse_value(tok[3])))  # short circuit at DC
        elif k == "d":
            elems.append(Elem(tok[0], "d", tok[1:3], None, tok[3].lower(), _kv(tok[4:])))
        elif k == "m":
            elems.append(Elem(tok[0], "m", tok[1:5], None, tok[5].lower(), _kv(tok[6:])))
        else:
            raise ValueError(f"unsupported element {tok[0]}")
    return Circuit(title, elems, models, analysis)


def is_ground(n: str) -> bool:
    return n.lower() in ("0", "gnd")


# --------------------------------------------------------------------------------------- mp MNA oracle
class Oracle:
    """Residual F(x; theta) = 0 of the deck in 50-digit arithmetic; x = [node voltages..., vsource branch currents...]."""

    def __init__(self, ckt: Circuit, vt=VT_300K15, gmin=GMIN, node_gmin=GMIN):
        self.c = ckt
        self.vt = mp.mpf(repr(vt))
        self.gmin = mp.mpf(repr(gmin))
        self.node_gmin = mp.mpf(repr(node_gmin))
        nodes = []
        for e in ckt.elems:
            for n in e.nodes:
                if not is_ground(n) and n.lower() not in nodes:
                    nodes.append(n.lower())
        self.nodes = nodes
        self.vsrc = [e.name.lower() for e in ckt.elems if e.kind in ("v", "l")]  # inductor = 0 V source at DC
        self.dim = len(nodes) + len(self.vsrc)
        self.theta = {}  # (devname_lower, param) -> mpf override
        self.strict = False

    def idx(self, n):
        return -1 if is_ground(n) else self.nodes.index(n.lower())

    def val(self, e: Elem, p: str, default=None):
        key = (e.name.lower(), p)
        if key in self.theta:
            return self.theta[key]
        return self.base_param(e, p, default)

    def base_param(self, e, p, default=None):
        if e.kind in "ri" and p in ("r", "dc"):
            return e.value
        if e.kind == "v" and p == "dc":
            return e.value
        mdl = self.c.models.get(e.model, {}) if e.model else {}
        src = e.params if p in e.params else mdl
        if e.kind == "m" and p in ("w", "l"):
            src = e.params
        if p in src:
            return parse_value(src[p])
        if default is None:
            raise KeyError(f"{e.name}.{p}")
        return mp.mpf(default)

    def residual(self, x):
        F = [mp.mpf(0)] * self.dim
        nn = len(self.nodes)

        def V(n):
            i = self.idx(n)
            return mp.mpf(0) if i < 0 else x[i]

        def add(n, cur):
            i = self.idx(n)
            if i >= 0:
                F[i] += cur

        for i in range(nn):
            F[i] += self.node_gmin * x[i]
        for e in self.c.elems:
            if e.kind == "r":
                i = (V(e.nodes[0]) - V(e.nodes[1])) / self.val(e, "r")
                add(e.nodes[0], i)
                add(e.nodes[1], -i)
            elif e.kind == "i":
                dc = self.val(e, "dc")
                add(e.nodes[0], dc)
                add(e.nodes[1], -dc)
            elif e.kind in ("v", "l"):
                k = nn + self.vsrc.index(e.name.lower())
                add(e.nodes[0], x[k])
                add(e.nodes[1], -x[k])
                F[k] += V(e.nodes[0]) - V(e.nodes[1]) - (self.val(e, "dc") if e.kind == "v" else 0)
            elif e.kind == "c":
                pass
            elif e.kind == "d":
                vd = V(e.nodes[0]) - V(e.nodes[1])
                n = self.val(e, "n", 1)
                IS = self.val(e, "is", "1e-14") * self.val(e, "area", 1)
                nvt = n * self.vt
                if vd >= -3 * nvt:
                    cur = IS * (mp.exp(vd / nvt) - 1) + self.gmin * vd
                else:  # SPICE3 reverse-bias branch
                    arg = 3 * nvt / (vd * mp.e)
                    cur = -IS * (1 + arg**3) + self.gmin * vd
                add(e.nodes[0], cur)
                add(e.nodes[1], -cur)
            elif e.kind == "m":
                cur, j_bd, j_bs = self.mos(e, V)
                add(e.nodes[0], cur)
                add(e.nodes[2], -cur)
                add(e.nodes[3], j_bd + j_bs)
                add(e.nodes[0], -j_bd)
                add(e.nodes[2], -j_bs)
            else:
                raise ValueError(e.kind)
        return F

    def mos(self, e, V):
        mdl = self.c.models[e.model]
        pol = mp.mpf(-1) if mdl["type"] == "pmos" else mp.mpf(1)
        vd, vg, vs, vb = (V(n) for n in e.nodes)
        vds_s = pol * (vd - vs)
        if vds_s >= 0:
            mode, vds, vgs, vbs = 1, vds_s, pol * (vg - vs), pol * (vb - vs)
        else:
            mode, vds, vgs, vbs = -1, -vds_s, pol * (vg - vd), pol * (vb - vd)
        kp = self.val(e, "kp", "2e-5")
        w, l = self.val(e, "w", "1e-4"), self.val(e, "l", "1e-4")
        lam = self.val(e, "lambda", 0)
        gam = self.val(e, "gamma", 0)
        phi = self.val(e, "phi", "0.6")
        vto = self.val(e, "vto", 0)
        von = pol * vto
        if gam > 0:
            if vbs > 0:
                if self.strict:
                    raise ValueError("oracle: forward-biased bulk (vbs > 0) is outside the modelled domain")
                vbs = mp.mpf(0)  # only reachable by Newton iterates; the converged point is checked with strict=True
            von = von + gam * (mp.sqrt(phi - vbs) - mp.sqrt(phi))
        vov = vgs - von
        beta = kp * w / l
        if vov <= 0:
            cd = mp.mpf(0)
        elif vds < vov:
            cd = beta * vds * (vov - vds / 2) * (1 + lam * vds)
        else:
            cd = beta / 2 * vov**2 * (1 + lam * vds)
        # bulk-drain and bulk-source junctions (anode = bulk for NMOS): ideal diodes of saturation current IS with
        # gmin in parallel; level-1 MOS has no other gmin (none between drain and source)
        if any(p in mdl for p in ("js", "cbd", "cbs", "cj", "cjsw")) or any(p in e.params for p in ("ad", "as", "pd", "ps")):
            raise ValueError("oracle: junction area/capacitance parameters are outside the modelled domain")
        isat = self.val(e, "is", "1e-14")

        def junction(v):
            return pol * (isat * (mp.exp(v / self.vt) - 1) + self.gmin * v)

        cur = pol * mode * cd
        return cur, junction(pol * (vb - vd)), junction(pol * (vb - vs))

    # ------------------------------------------------------------------------------------------ solve / derivs
    def jac(self, x, f0=None):
        n = self.dim
        J = mp.zeros(n, n)
        for j in range(n):
            h = mp.mpf(10) ** (-20) * max(1, abs(x[j]))
            xp, xm = list(x), list(x)
            xp[j] += h
            xm[j] -= h
            fp, fm = self.residual(xp), self.residual(xm)
            for i in range(n):
                J[i, j] = (fp[i] - fm[i]) / (2 * h)
        return J

    def solve(self, x0=None, tol=mp.mpf(10) ** (-30), maxit=400):
        n = self.dim
        x = list(x0) if x0 is not None else [mp.mpf(0)] * n
        for it in range(maxit):
            F = self.residual(x)
            J = self.jac(x)
            dx = mp.lu_solve(J, mp.matrix([-f for f in F]))
            # damped step: limit any voltage change to 0.3 V (diode-friendly)
            s = max(abs(d) for d in dx)
            lam = mp.mpf(1) if s <= 0.3 else mp.mpf("0.3") / s
            x = [x[i] + lam * dx[i] for i in range(n)]
            if s * lam < tol and lam == 1:
                break
        else:
            raise RuntimeError("oracle Newton did not converge")
        self.x = x
        self.strict = True
        try:
            self.residual(x)  # domain check at the converged point
        finally:
            self.strict = False
        return x

    def v(self, node):
        i = self.idx(node)
        return mp.mpf(0) if i < 0 else self.x[i]

    def sens(self, node, dev, param):
        """dV(node)/d(dev.param) at the solution by the implicit function theorem (mp.diff on the residual)."""
        key = (dev.lower(), param)
        e = next(e for e in self.c.elems if e.name.lower() == dev.lower())
        th0 = self.val(e, param, None) if True else None
        J = self.jac(self.x)
        h = mp.mpf(10) ** (-15) * abs(th0)  # rounding 1e-50*|F|/(h*|dF/dtheta|) stays < 1e-20 even for IS ~ 1e-14
        old = self.theta.get(key)
        self.theta[key] = th0 + h
        fp = self.residual(self.x)
        self.theta[key] = th0 - h
        fm = self.residual(self.x)
        if old is None:
            del self.theta[key]
        else:
            self.theta[key] = old
        dFdth = mp.matrix([(fp[i] - fm[i]) / (2 * h) for i in range(self.dim)])
        dx = mp.lu_solve(J, -dFdth)
        i = self.idx(node)
        return mp.mpf(0) if i < 0 else dx[i]


# ------------------------------------------------------------------------------------- deck parameter editing
def _fmt(v) -> str:
    return mp.nstr(mp.mpf(v), 17, min_fixed=1, max_fixed=0) if isinstance(v, mp.mpf) else repr(float(v))


def set_param(text: str, ckt: Circuit, dev: str, param: str, value) -> str:
    """Return deck text with dev.param set to `value` (resistor value / diode model IS / MOS W or model VTO ...)."""
    e = next(e for e in ckt.elems if e.name.lower() == dev.lower())
    lines = text.split("\n")
    val = repr(float(value))
    if e.kind == "r" and param == "r":
        for i, ln in enumerate(lines):
            tk = ln.split()
            if tk and tk[0].lower() == dev.lower():
                tk[3] = val
                lines[i] = " ".join(tk)
                return "\n".join(lines)
    if e.kind in "vi" and param == "dc":
        for i, ln in enumerate(lines):
            tk = ln.split()
            if tk and tk[0].lower() == dev.lower():
                j = 4 if tk[3].lower() == "dc" else 3
                tk[j] = val
                lines[i] = " ".join(tk)
                return "\n".join(lines)
    # instance parameter (w, l, area, ...) else model parameter
    for i, ln in enumerate(lines):
        tk = ln.split()
        if tk and tk[0].lower() == dev.lower() and any(t.lower().startswith(param + "=") for t in tk):
            lines[i] = re.sub(rf"(?i)\b{param}=\S+", f"{param}={val}", ln)
            return "\n".join(lines)
    mname = e.model
    for i, ln in enumerate(lines):
        tk = ln.split()
        if len(tk) > 2 and tk[0].lower() == ".model" and tk[1].lower() == mname:
            if re.search(rf"(?i)\b{param}=", ln):
                lines[i] = re.sub(rf"(?i)\b{param}=[^\s)]+", f"{param}={val}", ln)
                return "\n".join(lines)
    raise KeyError(f"cannot set {dev}.{param}")


def rel(a, b, floor=0.0):
    d = max(abs(b), floor)
    return abs(a - b) / d if d > 0 else (0.0 if a == b else math.inf)
