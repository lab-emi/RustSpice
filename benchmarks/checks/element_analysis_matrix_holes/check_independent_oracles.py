#!/usr/bin/env python3
"""Independent-oracle audit of the cap_xm_* decks.

For every cap_xm_* deck of benchmarks/cases/capability the script
  1. runs RustSpice (`rustspice run --raw`) and ngspice (black-box executable, tight/converged reference options
     exactly as the bench engine injects them) on the UNMODIFIED deck,
  2. evaluates a hand-written oracle that shares no code with either simulator
       - DC decks: 50-digit mpmath modified nodal analysis + Newton (R, V, I, G, D, L-as-short; Shockley diode with
         gmin = 1e-12 across the junction, Vt = k*T/q at 300.15 K with k = 1.38064852e-23, q = 1.6021766208e-19:
         these constants reproduce the junction voltage that ngspice-47 prints for a 1 mA / 1e-14 A diode to 3e-15
         relative, CODATA-2019 values would be 3.4e-7 off),
       - transient decks: hand-derived node equations integrated by scipy Radau with rtol = 1e-12 (nonlinear decks) or
         exact matrix exponentials of the piecewise-linear segments (linear RLC ladder), restarted at every source
         breakpoint; the closed form v = R*i(t) for the instance-name deck,
  3. checks (a) RustSpice vs oracle within the deck's own tolerance, (b) the ngspice REFERENCE run vs oracle within
     tolerance/4 (the reference is trustworthy), (c) mutation teeth: scaling the diode / MOS strength of the deck makes
     RustSpice leave the tolerance band (the comparison can see the nonlinear element), (d) for the known-gap deck:
     RustSpice is wrong against the oracle on exactly the documented probe and right on the control probe.

Exit status 0 = every audit line OK, 1 = at least one violation, 2 = environment problem (tool missing).

Environment: RUSTSPICE (default: target/release/rustspice of this repository), NGSPICE (default `ngspice`), EAMH_CPUS (taskset list, default: no pinning).  Peak memory: a few tens of MiB (dense 8x8 mpmath / 6x6 expm / 4-state Radau).
Usage: check_independent_oracles.py [--deck SUBSTR] [--verbose]
"""
import argparse, math, os, re, shutil, subprocess, sys, tempfile
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
# <repo>/modules/RustSpice/benchmarks/checks/element_analysis_matrix_holes/<script> -> parents[3] = <repo>/modules/RustSpice
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..'))
CASES = os.path.join(ROOT, 'benchmarks', 'cases', 'capability')
RS = os.environ.get('RUSTSPICE', os.path.join(ROOT, 'target', 'release', 'rustspice'))
NG = os.environ.get('NGSPICE', 'ngspice')
CPUS = os.environ.get('EAMH_CPUS', '')
PREFIX = 'cap_xm_'
TIGHT = 'reltol=1e-9 vntol=1e-12 abstol=1e-15'
REFTRAN = 'reltol=1e-6 abstol=1e-15 vntol=1e-9'

# ----------------------------------------------------------------------------------------------- raw file reader
def read_raw(path):
    data = open(path, 'rb').read(); pos = 0; plots = []
    while pos < len(data):
        i = data.find(b'Binary:\n', pos)
        if i < 0: break
        hdr = data[pos:i].decode('latin1').splitlines(); pos = i + 8
        nvar = npts = None; names = []; inv = False
        for ln in hdr:
            if ln.startswith('No. Variables:'): nvar = int(ln.split(':')[1])
            elif ln.startswith('No. Points:'): npts = int(ln.split(':')[1])
            elif ln.startswith('Variables:'): inv = True
            elif inv and ln.strip(): names.append(ln.split()[1].lower())
        arr = np.frombuffer(data, dtype='<f8', count=nvar * npts, offset=pos).reshape(npts, nvar); pos += 8 * nvar * npts
        plots.append({'x': arr[:, 0].copy(), 'v': {n: arr[:, k].copy() for k, n in enumerate(names)}})
    return plots[-1]

def pin(cmd):
    return (['taskset', '-c', CPUS] if CPUS else []) + cmd

def run_rs(deck, work, tag='rs'):
    raw = os.path.join(work, tag + '.raw')
    p = subprocess.run(pin([RS, 'run', deck, '--raw', raw]), capture_output=True, text=True, timeout=120)
    if p.returncode != 0: raise RuntimeError('rustspice rc=%d: %s' % (p.returncode, p.stderr[-300:]))
    return read_raw(raw)

def run_ng(text, work, tag, options=None, tmax=None):
    t = text
    if tmax is not None:
        t = re.sub(r'(?im)^\.tran\s+(\S+)\s+(\S+).*$', lambda m: '.tran %s %s 0 %.6g' % (m.group(1), m.group(2), tmax), t)
    if options:
        t = re.sub(r'(?im)^\.end\s*$', '.options %s\n.end' % options, t, count=1)
    d = os.path.join(work, tag + '.sp'); open(d, 'w').write(t); raw = os.path.join(work, tag + '.raw')
    p = subprocess.run(pin([NG, '-b', '-r', raw, d]), capture_output=True, text=True, timeout=300)
    if p.returncode != 0 or not os.path.exists(raw): raise RuntimeError('ngspice rc=%d' % p.returncode)
    return read_raw(raw)

def meta(text):
    m = {}
    for ln in text.splitlines()[1:]:
        t = ln.strip()
        if t.startswith('*') and 'bench.' in t:
            k, _, v = t.lstrip('* ').partition(':'); m[k.strip()[6:]] = v.strip()
    return m

def num(tok):
    s = {'meg': 1e6, 'g': 1e9, 't': 1e12, 'k': 1e3, 'm': 1e-3, 'u': 1e-6, 'n': 1e-9, 'p': 1e-12, 'f': 1e-15}
    t = tok.lower(); mm = re.match(r'^([-+]?[0-9.]+(?:e[-+]?[0-9]+)?)(meg|[gtkmunpf])?', t)
    return float(mm.group(1)) * (s[mm.group(2)] if mm.group(2) else 1.0)

# ----------------------------------------------------------------------------------------------- DC oracle (mpmath)
import mpmath as mp
mp.mp.dps = 50
KB = mp.mpf('1.38064852e-23'); QE = mp.mpf('1.6021766208e-19'); TK = mp.mpf('300.15'); VT = KB * TK / QE

def solve_dc(elems, x0=None, gmin=mp.mpf('1e-12')):
    def nd(e):
        return e[1:3] if e[0] in 'RVILD' else e[1:5]
    nodes = []
    for e in elems:
        for n in nd(e):
            if n != '0' and n not in nodes: nodes.append(n)
    br = [e for e in elems if e[0] in 'VL']; nn = len(nodes); N = nn + len(br)
    idx = {n: i for i, n in enumerate(nodes)}; bidx = {e[3]: nn + i for i, e in enumerate(br)}
    x = mp.matrix(N, 1) if x0 is None else mp.matrix(x0)
    for it in range(400):
        F = mp.matrix(N, 1); J = mp.matrix(N, N)
        vof = lambda n: x[idx[n]] if n in idx else mp.mpf(0)
        def aF(n, v):
            if n in idx: F[idx[n]] += v
        def aJ(n, m, v):
            if n in idx and m in idx: J[idx[n], idx[m]] += v
        for e in elems:
            k = e[0]
            if k == 'R':
                g = 1 / mp.mpf(e[3]); i_ = g * (vof(e[1]) - vof(e[2])); aF(e[1], i_); aF(e[2], -i_)
                aJ(e[1], e[1], g); aJ(e[1], e[2], -g); aJ(e[2], e[1], -g); aJ(e[2], e[2], g)
            elif k == 'I':      # current flows from n1 through the source to n2
                v = mp.mpf(e[3]); aF(e[1], v); aF(e[2], -v)
            elif k == 'G':      # gm*v(c+,c-) flows n+ -> n- through the element
                gm = mp.mpf(e[5]); i_ = gm * (vof(e[3]) - vof(e[4])); aF(e[1], i_); aF(e[2], -i_)
                aJ(e[1], e[3], gm); aJ(e[1], e[4], -gm); aJ(e[2], e[3], -gm); aJ(e[2], e[4], gm)
            elif k == 'D':
                IS, n = mp.mpf(e[3]), mp.mpf(e[4]); vd = vof(e[1]) - vof(e[2]); ex = mp.e ** (vd / (n * VT))
                i_ = IS * (ex - 1) + gmin * vd; gd = IS * ex / (n * VT) + gmin
                aF(e[1], i_); aF(e[2], -i_); aJ(e[1], e[1], gd); aJ(e[1], e[2], -gd); aJ(e[2], e[1], -gd); aJ(e[2], e[2], gd)
            else:               # V or L: branch current flows into the first node through the element to the second
                bi = bidx[e[3]]; ib = x[bi]; aF(e[1], ib); aF(e[2], -ib)
                if e[1] in idx: J[idx[e[1]], bi] += 1; J[bi, idx[e[1]]] += 1
                if e[2] in idx: J[idx[e[2]], bi] -= 1; J[bi, idx[e[2]]] -= 1
                F[bi] = vof(e[1]) - vof(e[2]) - (mp.mpf(e[4]) if k == 'V' else 0)
        dx = mp.lu_solve(J, -F); st = max(abs(dx[i]) for i in range(N)); lim = mp.mpf('0.3')
        if st > lim: dx = dx * (lim / st)
        x = x + dx
        if st < mp.mpf('1e-30'): break
    else:
        raise RuntimeError('oracle Newton failed')
    out = {'v(%s)' % n: float(x[idx[n]]) for n in nodes}
    out.update({'i(%s)' % nme: float(x[i]) for nme, i in bidx.items()})
    out['_x'] = x
    return out

def dc_sweep(build, axis):
    res = {}; x0 = None
    for a in axis:
        r = solve_dc(build(a), x0=x0); x0 = r['_x']
        for k, v in r.items():
            if k != '_x': res.setdefault(k, []).append(v)
    return {k: np.array(v) for k, v in res.items()}

DM = 1e-14
def dc_l_v(v):   return [('V', 'a', '0', 'v1', v), ('R', 'a', 'b', 1000), ('L', 'b', 'c', 'l1'), ('R', 'c', 'd', 500), ('D', 'd', '0', DM, 1)]
def dc_l_i(i):   return [('I', '0', 'a', i), ('L', 'a', 'b', 'l1'), ('R', 'b', '0', 1000), ('D', 'b', '0', DM, 1)]
def dc_g(v):     return [('V', 'in', '0', 'v1', v), ('R', 'in', 'c', 1000), ('R', 'c', '0', 1000), ('G', '0', 'out', 'c', '0', 2e-3), ('R', 'out', '0', 1000), ('D', 'out', '0', DM, 1)]
def dc_sub(v):   return [('V', 'in', '0', 'v1', v), ('R', 'in', 'x1.k', 2000), ('D', 'x1.k', 'm', DM, 1), ('R', 'm', 'x2.k', 2000), ('D', 'x2.k', 'o', DM, 1), ('R', 'o', '0', 1000)]
def dc_subg(v):
    e = [('V', 'in', '0', 'v1', v)]; prev = 'in'
    for k, out in ((1, 'n1'), (2, 'n2'), (3, 'n3')):
        m = 'x%d.m' % k
        e += [('R', prev, m, 1000), ('R', m, '0', 1000), ('G', '0', out, m, '0', 4e-3), ('R', out, '0', 1000)]; prev = out
    return e + [('D', 'n3', '0', DM, 1), ('R', 'n3', '0', 2000)]

# ----------------------------------------------------------------------------------------------- transient oracles
from scipy.integrate import solve_ivp
from scipy.linalg import expm
K_B = 1.38064852e-23; Q_E = 1.6021766208e-19; VTF = K_B * 300.15 / Q_E; GMIN = 1e-12
def idio(v, IS=1e-14): return IS * np.expm1(v / VTF) + GMIN * v
def gdio(v, IS=1e-14): return IS * np.exp(v / VTF) / VTF + GMIN
def pulse(t, v1, v2, td, tr, tf, pw, per):
    if t < td: return v1
    tt = (t - td) % per
    if tt < tr: return v1 + (v2 - v1) * tt / tr
    if tt < tr + pw: return v2
    if tt < tr + pw + tf: return v2 + (v1 - v2) * (tt - tr - pw) / tf
    return v1

def integrate(rhs, y0, ts, bps=(), rtol=1e-12, atol=1e-15, jac=None):
    ts = np.asarray(ts, float); cuts = sorted(set([0.0, ts[-1]] + [b for b in bps if 0 < b < ts[-1]]))
    out = np.empty((len(y0), len(ts))); y = np.array(y0, float)
    for a, b in zip(cuts[:-1], cuts[1:]):
        sel = np.where((ts >= a) & ((ts < b) if b != cuts[-1] else (ts <= b)))[0]
        s = solve_ivp(rhs, (a, b), y, method='Radau', t_eval=np.unique(np.append(ts[sel], b)), rtol=rtol, atol=atol, jac=jac)
        if not s.success: raise RuntimeError('ODE: ' + s.message)
        for j in sel: out[:, j] = s.y[:, int(np.argmin(abs(s.t - ts[j])))]
        y = s.y[:, -1]
    return out

def tr_g_sine(ts):         # VCCS 2 mS from 10 MHz sine into 1k || 20p || diode
    r = lambda t, y: [(2e-3 * math.sin(2 * math.pi * 10e6 * t) - y[0] / 1e3 - idio(y[0])) / 20e-12]
    return {'v(out)': integrate(r, [0.0], ts, jac=lambda t, y: [[(-1e-3 - gdio(y[0])) / 20e-12]])[0]}

def tr_i_pulse(ts):        # I1 pulse (-0.5m..1m) into 2k || 50p, plus 500 ohm + diode branch
    from scipy.optimize import brentq
    R1, C, R2 = 2e3, 50e-12, 500.0
    iw = lambda t: pulse(t, -0.5e-3, 1e-3, 20e-9, 5e-9, 5e-9, 80e-9, 200e-9)
    vb = lambda va: brentq(lambda v: (va - v) / R2 - idio(v), -3.0, max(va, 1e-9) + 1.0, xtol=1e-18, rtol=1e-15)
    r = lambda t, y: [(iw(t) - y[0] / R1 - (y[0] - vb(y[0])) / R2) / C]
    ya = integrate(r, [-0.5e-3 * R1], ts, bps=[20e-9, 25e-9, 105e-9, 110e-9])[0]
    return {'v(a)': ya, 'v(b)': np.array([vb(v) for v in ya])}

def tr_l_decay(ts):        # 3 V pulse -> R1 30 -> L1 2u -> diode; state = diode voltage (i_L = Id(vb))
    R1, L = 30.0, 2e-6; vin = lambda t: pulse(t, 0, 3, 10e-9, 1e-9, 1e-9, 100e-9, 300e-9)
    r = lambda t, y: [(vin(t) - R1 * idio(y[0]) - y[0]) / (L * gdio(y[0]))]
    vb = integrate(r, [0.0], ts, bps=[10e-9, 11e-9, 111e-9, 112e-9], rtol=1e-11, atol=1e-14)[0]
    il = idio(vb); va = np.array([vin(t) for t in ts]) - R1 * il
    return {'v(a)': va, 'i(l1)': il, 'i(v1)': -il}

def mos(vgs, vds, vto, kp, w, l, lam):   # level-1 (Shichman-Hodges), n-channel orientation
    b = kp * w / l; vov = vgs - vto
    if vov <= 0: return 0.0
    return b * (vov * vds - 0.5 * vds * vds) * (1 + lam * vds) if vds < vov else 0.5 * b * vov * vov * (1 + lam * vds)
def tr_cmos(ts, kpn=120e-6, kpp=60e-6):
    VDD = 3.3; C = 20e-15; vin = lambda t: pulse(t, 0.0, 3.3, 0.2e-9, 0.1e-9, 0.1e-9, 2e-9, 4e-9)
    def r(t, y):
        g = [vin(t)] + list(y[:3]); return [(mos(VDD - g[k], VDD - y[k], 0.7, kpp, 20e-6, 1e-6, 0.02) - mos(g[k], y[k], 0.7, kpn, 10e-6, 1e-6, 0.02)) / C for k in range(4)]
    y = integrate(r, [VDD, 0, VDD, 0], ts, bps=[0.2e-9, 0.3e-9, 2.3e-9, 2.4e-9], rtol=1e-10, atol=1e-13)
    ipdd = np.zeros(len(ts))
    for j, t in enumerate(ts):
        g = [vin(t)] + [y[k, j] for k in range(3)]
        ipdd[j] = -sum(mos(VDD - g[k], VDD - y[k, j], 0.7, kpp, 20e-6, 1e-6, 0.02) for k in range(4))
    return {'v(n1)': y[0], 'v(n2)': y[1], 'v(n3)': y[2], 'v(n4)': y[3], 'i(vdd)': ipdd}

def tr_ladder(ts):         # 3 x (R 5, L 100n, C 100p) + 50 ohm load, exact exponentials of the PWL segments
    R, L, C, RL = 5.0, 100e-9, 100e-12, 50.0
    A = np.zeros((6, 6)); B = np.zeros(6)
    for k in range(3):
        A[k, k] = -R / L; A[k, 3 + k] = -1 / L
        if k == 0: B[k] = 1 / L
        else: A[k, 3 + k - 1] = 1 / L
        A[3 + k, k] = 1 / C
        if k < 2: A[3 + k, k + 1] = -1 / C
        else: A[3 + k, 3 + k] = -1 / (RL * C)
    bt = [0, 5e-9, 7e-9, 47e-9, 49e-9, 100e-9]; bu = [0, 0, 1, 1, 0, 0]
    ts = np.asarray(ts, float); out = {k: np.empty(len(ts)) for k in ('v(n1)', 'v(n2)', 'v(n3)', 'i(v1)', 'v(x1.x1.m)', 'v(x3.x1.m)')}
    xs = [np.zeros(6)]
    segs = [(bt[k], bt[k + 1], bu[k], (bu[k + 1] - bu[k]) / (bt[k + 1] - bt[k])) for k in range(len(bt) - 1)]
    def adv(x, u0, s, dt):
        M = np.zeros((8, 8)); M[:6, :6] = A; M[:6, 6] = B; M[6, 7] = s
        z = expm(M * dt) @ np.concatenate([x, [u0, 1.0]]); return z[:6], z[6]
    for k in range(len(segs) - 1): xs.append(adv(xs[-1], segs[k][2], segs[k][3], segs[k][1] - segs[k][0])[0])
    for j, t in enumerate(ts):
        k = max(i for i, s_ in enumerate(segs) if s_[0] <= t) if t < bt[-1] else len(segs) - 1
        a, b, u0, s = segs[k]; x, u = adv(xs[k], u0, s, min(t, b) - a)
        out['v(n1)'][j], out['v(n2)'][j], out['v(n3)'][j], out['i(v1)'][j] = x[3], x[4], x[5], -x[0]
        out['v(x1.x1.m)'][j] = u - R * x[0]; out['v(x3.x1.m)'][j] = x[4] - R * x[2]
    return out

def tr_subsource(ts):      # V1 pulse -> R1 -> out ; I1 sin -> b ; R2 b-out ; R3, C1 on out ; C2 on b
    iw = lambda t: 1e-3 * math.sin(2 * math.pi * 10e6 * t); va = lambda t: pulse(t, 0, 2, 10e-9, 5e-9, 5e-9, 40e-9, 100e-9)
    def r(t, y):
        vb, vo = y; return [(iw(t) - (vb - vo) / 2e3) / 5e-12, ((va(t) - vo) / 1e3 + (vb - vo) / 2e3 - vo / 1e3) / 20e-12]
    y = integrate(r, [0.0, 0.0], ts, bps=[10e-9, 15e-9, 55e-9, 60e-9])
    return {'v(a)': np.array([va(t) for t in ts]), 'v(b)': y[0], 'v(out)': y[1]}

def tr_instname(ts):       # v(o) = 1k * 1m * sin(2 pi 10 MHz t), algebraic
    v = 1.0 * np.sin(2 * np.pi * 10e6 * np.asarray(ts)); return {'v(o1)': v, 'v(o2)': v}

# ----------------------------------------------------------------------------------------------- audit logic
DECKS = {
    'l_dc_sweep_diode':            ('dc', lambda ax: dc_sweep(dc_l_v, ax), {}),
    'l_dc_sweep_isrc_diode':       ('dc', lambda ax: dc_sweep(dc_l_i, ax), {}),
    'g_dc_sweep_diode_load':       ('dc', lambda ax: dc_sweep(dc_g, ax), {}),
    'sub_dc_sweep_diode_divider':  ('dc', lambda ax: dc_sweep(dc_sub, ax), {'v(x1.k)': 'v(x1.k)'}),
    'sub_g_internal_ctrl_dc_sweep': ('dc', lambda ax: dc_sweep(dc_subg, ax), {}),
    'g_tran_sine_diode_rc':        ('tran', tr_g_sine, {}),
    'i_pulse_diode_rc_tran':       ('tran', tr_i_pulse, {}),
    'l_tran_diode_decay':          ('tran', tr_l_decay, {}),
    'sub_cmos_chain4_tran':        ('tran', tr_cmos, {}),
    'sub_rlc_ladder_tran':         ('tran', tr_ladder, {}),
    'sub_source_pulse_sin_tran':   ('tran', tr_subsource, {}),
    'sub_instance_name_waveform_tran': ('tran', tr_instname, {}),
}
# decks whose RustSpice result is KNOWN to be wrong on the listed probes (bench.known_gap): audit expects the error.
# Empty since 2026-10-28: sub_instance_name_waveform_tran (instance name containing PULSE) was closed by the
# keyword-based source reader and must now match the oracle like every other probe.
GAP_PROBES = {}
# teeth: (regex, replacement) mutations of the deck text; RustSpice on the mutated deck must leave the tolerance band
TEETH = {
    'l_dc_sweep_diode': [(r'IS=1e-14', 'IS=1e-13')], 'l_dc_sweep_isrc_diode': [(r'IS=1e-14', 'IS=1e-13')],
    'g_dc_sweep_diode_load': [(r'IS=1e-14', 'IS=1e-13')], 'sub_dc_sweep_diode_divider': [(r'IS=1e-14', 'IS=1e-13')],
    'sub_g_internal_ctrl_dc_sweep': [(r'IS=1e-14', 'IS=1e-13')],
    'g_tran_sine_diode_rc': [(r'IS=1e-14', 'IS=1e-12')], 'i_pulse_diode_rc_tran': [(r'IS=1e-14', 'IS=1e-13')],
    'l_tran_diode_decay': [(r'IS=1e-14', 'IS=1e-13')],
    'sub_cmos_chain4_tran': [(r'KP=120u', 'KP=132u')],
    'sub_rlc_ladder_tran': [(r'L1 m b 100n', 'L1 m b 105n')],
    'sub_source_pulse_sin_tran': [(r'C2 b 0 5p', 'C2 b 0 6p')],
}

def main():
    ap = argparse.ArgumentParser(); ap.add_argument('--deck', default=''); ap.add_argument('--verbose', action='store_true'); a = ap.parse_args()
    for tool in (RS, NG):
        if not (os.path.isfile(tool) or shutil.which(tool)): print('missing tool', tool); return 2
    work = tempfile.mkdtemp(prefix='eamh_oracle_'); bad = 0
    try:
        for stem, (kind, orc, _) in DECKS.items():
            if a.deck not in stem: continue
            path = os.path.join(CASES, PREFIX + stem + '.sp'); text = open(path).read(); md = meta(text)
            probes = [p.strip() for p in md['probes'].split(',')]
            gap = GAP_PROBES.get(stem, [])
            rs = run_rs(path, work)
            if kind == 'dc':
                ng = run_ng(text, work, 'ng', options=TIGHT); nga = run_ng(text, work, 'ngas')
                tabs = num(md.get('tol.abs', '1e-9')); trel = float(md.get('tol.rel', '1e-6'))
                o = orc(list(rs['x'])); o2 = orc(list(ng['x'])); o3 = orc(list(nga['x']))
                for p in probes:
                    ref = o[p]; tol = tabs + trel * np.abs(ref)
                    e_rs = np.max(np.abs(rs['v'][p] - ref) / tol); e_ng = np.max(np.abs(ng['v'][p] - o2[p]) / (tabs + trel * np.abs(o2[p])))
                    ok = e_rs <= 1.0 and e_ng <= 0.25
                    bad += (not ok)
                    print('%-4s %-38s %-10s rs/oracle %.3g%% of tol, ngref/oracle %.3g%% of tol (max |rs-orc| %.2e, max |ngref-orc| %.2e, max |ng as-is - orc| %.2e)' % ('OK' if ok else 'FAIL', stem, p, 100 * e_rs, 100 * e_ng, np.max(np.abs(rs['v'][p] - ref)), np.max(np.abs(ng['v'][p] - o2[p])), np.max(np.abs(nga['v'][p] - o3[p]))))
                nsw = len(rs['x'])
            else:
                tstep = num(re.search(r'(?im)^\.tran\s+(\S+)', text).group(1)); tol = float(md.get('tol.norm', '5e-3'))
                ng = run_ng(text, work, 'ngref', options=REFTRAN, tmax=tstep / 20); nga = run_ng(text, work, 'ngas')
                o = orc(rs['x']); o2 = orc(ng['x']); o3 = orc(nga['x'])
                for p in probes:
                    sw = max(np.ptp(o[p]), float(md.get('floor', '1e-6')))
                    e_rs = np.max(np.abs(rs['v'][p] - o[p])) / sw; e_ng = np.max(np.abs(ng['v'][p] - o2[p])) / sw
                    if p in gap:
                        ok = e_rs >= 20 * tol and e_ng <= tol / 4; tag = 'GAP'
                    else:
                        ok = e_rs <= tol and e_ng <= tol / 4; tag = 'OK'
                    bad += (not ok)
                    e_as = np.max(np.abs(nga['v'][p] - o3[p])) / sw
                    print('%-4s %-38s %-10s rs/oracle %.3e, ngref/oracle %.3e, ng as-is/oracle %.3e (tol.norm %.1e, swing %.4g)%s' % (tag if ok else 'FAIL', stem, p, e_rs, e_ng, e_as, tol, sw, '  [documented gap: RustSpice must be wrong here]' if p in gap else ''))
                nsw = len(rs['x'])
            # teeth: mutate the nonlinear element / a dynamic element and require RustSpice to leave the band
            for k, (pat, rep) in enumerate(TEETH.get(stem, [])):
                mt = re.sub(pat, rep, text, count=1)
                if mt == text: print('FAIL teeth pattern not found', stem, pat); bad += 1; continue
                mp_ = os.path.join(work, 'mut.sp'); open(mp_, 'w').write(mt)
                try: rm = run_rs(mp_, work, 'rsmut')
                except Exception as ex: print('FAIL teeth run', stem, ex); bad += 1; continue
                worst = 0.0
                for p in probes:
                    if p in gap: continue
                    if kind == 'dc':
                        ref = ng['v'][p]; t = tabs + trel * np.abs(ref); worst = max(worst, np.max(np.abs(rm['v'][p] - ref) / t))
                    else:
                        sw = max(np.ptp(o[p]), float(md.get('floor', '1e-6')))
                        om = np.interp(rm['x'], ng['x'], ng['v'][p]); worst = max(worst, np.max(np.abs(rm['v'][p] - om)) / sw / tol)
                ok = worst >= 10.0 if stem not in GAP_PROBES else True
                bad += (not ok)
                print('%-4s %-38s teeth %-22s mutated RustSpice at %.3g x tolerance (needs >= 10)' % ('OK' if ok else 'FAIL', stem, pat + '->' + rep, worst))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    print('RESULT: %s (%d violation(s))' % ('PASS' if bad == 0 else 'FAIL', bad))
    return 0 if bad == 0 else 1

if __name__ == '__main__':
    sys.exit(main())
