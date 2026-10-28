"""Shared helpers of the numbering_order_invariance check_*.py scripts (stdlib + numpy + scipy only).

Independent oracle: exact matrix-exponential solution of a linear R/C/L/V/I circuit with a piecewise-linear
(PULSE) drive, solved in dense f64; it is validated by the scripts themselves against closed forms
(`python3 _noi_common.py` runs the self-test: RC ramp response to 1e-14, series RLC step to the rise-time limit).
ngspice is used ONLY as a black-box executable (AGENTS.md Rule 1).
"""
import os, sys, re, json, math, random, shlex, subprocess, hashlib, time
import pathlib

# <repo>/modules/RustSpice/benchmarks/checks/numbering_order_invariance/_noi_common.py -> parents[3] = <repo>/modules/RustSpice
ROOT = pathlib.Path(__file__).resolve().parents[3]
CASES = ROOT / 'benchmarks' / 'cases'
DEFAULT_RUSTSPICE = os.environ.get('RUSTSPICE', str(ROOT / 'target' / 'release' / 'rustspice'))
DEFAULT_NGSPICE = os.environ.get('NGSPICE', 'ngspice')
"""Minimal ngspice binary rawfile reader (real, last plot) -- independent of both simulators."""
import numpy as np

def read_raw(path, plot=-1):
    data = open(path, 'rb').read()
    plots = []
    pos = 0
    while pos < len(data):
        # header is ascii up to "Binary:\n"
        i = data.find(b'Binary:\n', pos)
        if i < 0:
            break
        hdr = data[pos:i].decode('latin1')
        body = i + len('Binary:\n')
        lines = hdr.split('\n')
        nvars = npts = None
        names = []
        invars = False
        flags = ''
        title = ''
        for ln in lines:
            if ln.startswith('No. Variables:'):
                nvars = int(ln.split(':')[1])
            elif ln.startswith('No. Points:'):
                npts = int(ln.split(':')[1])
            elif ln.startswith('Flags:'):
                flags = ln.split(':', 1)[1].strip()
            elif ln.startswith('Plotname:'):
                title = ln.split(':', 1)[1].strip()
            elif ln.startswith('Variables:'):
                invars = True
                rest = ln.split(':', 1)[1].split()
                if rest:
                    names.append(rest[1].lower())
            elif invars and ln.strip():
                parts = ln.split()
                if len(parts) >= 2 and parts[0].isdigit():
                    names.append(parts[1].lower())
        assert 'complex' not in flags, 'complex raw not supported'
        sz = nvars * npts * 8
        arr = np.frombuffer(data[body:body + sz], dtype='<f8').reshape(npts, nvars)
        plots.append((title, {n: arr[:, k].copy() for k, n in enumerate(names)}))
        pos = body + sz
    return plots[plot][1]

"""Netlist generator, permuter and independent exact oracle for the numbering-invariance decks.

Element model: dict(name, kind, nodes, tail)  where tail is the text after the node names.
The oracle re-parses the written deck text, so it checks the actual file, not the generator state.
"""
import numpy as np
from scipy.linalg import expm

def si(tok):
    tok = tok.strip().lower()
    m = re.fullmatch(r'([-+]?[0-9]*\.?[0-9]+(?:e[-+]?[0-9]+)?)(meg|mil|[tgkmunpf])?[a-z]*', tok)
    if not m:
        raise ValueError('bad number ' + tok)
    mult = {None: 1, 't': 1e12, 'g': 1e9, 'meg': 1e6, 'k': 1e3, 'm': 1e-3, 'u': 1e-6, 'n': 1e-9, 'p': 1e-12, 'f': 1e-15, 'mil': 25.4e-6}
    return float(m.group(1)) * mult[m.group(2)]

def fmt(x):
    """Shortest exact-ish decimal that both tools parse identically."""
    return repr(float(x)) if not float(x).is_integer() else str(int(x))

# ---------------------------------------------------------------- generators
def rc_ladder(n, R='1k', C='1p', src='PULSE(0 1 1n 1n 1n 8n 100n)'):
    els = [dict(name='V1', kind='V', nodes=['in', '0'], tail=src)]
    prev = 'in'
    for k in range(1, n + 1):
        els.append(dict(name=f'R{k}', kind='R', nodes=[prev, f'a{k}'], tail=R))
        els.append(dict(name=f'C{k}', kind='C', nodes=[f'a{k}', '0'], tail=C))
        prev = f'a{k}'
    return els

def lc_ladder(n, L='1n', C='1p', Rs='50', Rl='50', src='PULSE(0 1 0 100p 100p 1n 100n)'):
    els = [dict(name='V1', kind='V', nodes=['s', '0'], tail=src),
           dict(name='Rs', kind='R', nodes=['s', 'a0'], tail=Rs)]
    for k in range(1, n + 1):
        els.append(dict(name=f'L{k}', kind='L', nodes=[f'a{k-1}', f'a{k}'], tail=L))
        els.append(dict(name=f'C{k}', kind='C', nodes=[f'a{k}', '0'], tail=C))
    els.append(dict(name='Rl', kind='R', nodes=[f'a{n}', '0'], tail=Rl))
    return els

def res_mesh(m, R='1', rng=None, rlo=None, rhi=None, srcs=None):
    """m x m resistor mesh, grounded by one resistor, current sources injected."""
    els = []
    def nm(i, j): return f'm{i}_{j}'
    for i in range(m):
        for j in range(m):
            if j + 1 < m:
                els.append(dict(name=f'Rh{i}_{j}', kind='R', nodes=[nm(i, j), nm(i, j + 1)], tail=R))
            if i + 1 < m:
                els.append(dict(name=f'Rv{i}_{j}', kind='R', nodes=[nm(i, j), nm(i + 1, j)], tail=R))
    return els

# ---------------------------------------------------------------- permutation
def permute(els, seed, keep=('0',), relabel=True, shuffle=True, probes=()):
    """Return (new elements, mapping old->new node label, new probe list). Deterministic in seed."""
    rng = random.Random(seed)
    nodes = []
    for e in els:
        for nd in e['nodes']:
            if nd not in keep and nd not in nodes:
                nodes.append(nd)
    labels = list(range(len(nodes)))
    mp = {nd: nd for nd in keep}
    if relabel:
        rng.shuffle(labels)
        for nd, k in zip(nodes, labels):
            mp[nd] = f'q{k}'
    else:
        for nd in nodes:
            mp[nd] = nd
    new = [dict(e, nodes=[mp[nd] for nd in e['nodes']]) for e in els]
    if shuffle:
        rng.shuffle(new)
    return new, mp

def write_deck(path, title, meta, els, cards, end=True, extra_after=()):
    """meta: list of (key, value). cards: list of directive strings (.tran ...)."""
    lines = [title]
    for k, v in meta:
        lines.append(f'* bench.{k}: {v}')
    for e in els:
        lines.append(' '.join([e['name']] + e['nodes'] + [e['tail']]))
    lines.extend(cards)
    if end:
        lines.append('.end')
    open(path, 'w').write('\n'.join(lines) + '\n')

# ---------------------------------------------------------------- parse deck back (oracle input)
def parse_deck(path):
    els = []
    tran = None
    for raw in open(path).read().split('\n')[1:]:
        ln = raw.strip()
        if not ln or ln.startswith('*'):
            continue
        low = ln.lower()
        if low.startswith('.tran'):
            t = ln.split()
            tran = (si(t[1]), si(t[2]))
            continue
        if low.startswith('.'):
            continue
        tok = ln.split(None, 3)
        nm = tok[0]
        kind = nm[0].upper()
        if kind in 'RCL':
            els.append(dict(name=nm, kind=kind, nodes=[tok[1], tok[2]], val=si(tok[3].split()[0])))
        elif kind in 'VI':
            els.append(dict(name=nm, kind=kind, nodes=[tok[1], tok[2]], wave=tok[3]))
        else:
            raise ValueError('oracle: unsupported card ' + ln)
    return els, tran

def parse_wave(spec):
    s = spec.strip()
    m = re.match(r'(?i)^pulse\s*\(([^)]*)\)', s)
    if m:
        a = [si(x) for x in m.group(1).replace(',', ' ').split()]
        a += [0.0] * (7 - len(a))
        return ('pulse', a)
    m = re.match(r'(?i)^(?:dc\s+)?([-+0-9.eE]+[a-zA-Z]*)\s*$', s)
    if m:
        return ('dc', si(m.group(1)))
    raise ValueError('wave ' + spec)

def pulse_pw_points(a, tstop):
    v1, v2, td, tr, tf, pw, per = a
    pts = [(0.0, v1)]
    t0 = td
    pts += [(td, v1), (td + tr, v2), (td + tr + pw, v2), (td + tr + pw + tf, v1)]
    return pts

def src_value(w, t):
    if w[0] == 'dc':
        return w[1]
    pts = pulse_pw_points(w[1], 1e300)
    if t <= pts[0][0]:
        return pts[0][1]
    for (t0, y0), (t1, y1) in zip(pts, pts[1:]):
        if t0 <= t <= t1:
            if t1 == t0:
                return y1
            return y0 + (y1 - y0) * (t - t0) / (t1 - t0)
    return pts[-1][1]

def breakpoints(w):
    if w[0] == 'dc':
        return []
    return [p[0] for p in pulse_pw_points(w[1], 1e300)]

def oracle_tran(path, probes=None):
    """Exact (matrix-exponential, piecewise-linear input) transient on the grid k*tstep.
    Single PULSE/DC source set; returns (t, dict name->array). f64 dense."""
    els, tran = parse_deck(path)
    tstep, tstop = tran
    for e in els:
        if e['kind'] in 'VI':
            w = parse_wave(e['wave'])
            if w[0] == 'pulse':
                assert w[1][6] == 0 or w[1][2] + w[1][6] > tstop, 'oracle supports a single pulse'
    nodes = []
    for e in els:
        for nd in e['nodes']:
            if nd != '0' and nd not in nodes:
                nodes.append(nd)
    nid = {nd: k for k, nd in enumerate(nodes)}
    nn = len(nodes)
    br = [e for e in els if e['kind'] in 'VL']
    bid = {e['name'].lower(): nn + k for k, e in enumerate(br)}
    N = nn + len(br)
    M = np.zeros((N, N)); K = np.zeros((N, N))
    bconst = np.zeros(N)
    bsrc = []   # (row, sign, wave)
    def ix(nd): return nid.get(nd, None)
    for e in els:
        a, b = (ix(x) for x in e['nodes'])
        if e['kind'] == 'R':
            g = 1.0 / e['val']
            for (p, q, s) in ((a, a, 1), (b, b, 1), (a, b, -1), (b, a, -1)):
                if p is not None and q is not None:
                    K[p, q] += s * g
        elif e['kind'] == 'C':
            c = e['val']
            for (p, q, s) in ((a, a, 1), (b, b, 1), (a, b, -1), (b, a, -1)):
                if p is not None and q is not None:
                    M[p, q] += s * c
        elif e['kind'] == 'L':
            r = bid[e['name'].lower()]
            if a is not None: K[a, r] += 1; K[r, a] -= 1
            if b is not None: K[b, r] -= 1; K[r, b] += 1
            M[r, r] += e['val']
        elif e['kind'] == 'V':
            r = bid[e['name'].lower()]
            if a is not None: K[a, r] += 1; K[r, a] += 1
            if b is not None: K[b, r] -= 1; K[r, b] -= 1
            bsrc.append((r, 1.0, parse_wave(e['wave'])))
        elif e['kind'] == 'I':
            w = parse_wave(e['wave'])
            if a is not None: bsrc.append((a, -1.0, w))
            if b is not None: bsrc.append((b, +1.0, w))
    waves = {}
    def bvec(t):
        b = np.zeros(N)
        for (r, s, w) in bsrc:
            b[r] += s * src_value(w, t)
        return b
    # decompose b(t) = sum_k u_k(t) * e_k : treat each distinct source separately via linearity
    # Support: all sources share breakpoints; handle general by segmenting at union of breakpoints and using
    # per-segment slope vector (b(t1)-b(t0))/(t1-t0).
    diff = [i for i in range(N) if M[i, i] != 0.0]
    assert np.allclose(M - np.diag(np.diag(M)), 0), 'oracle needs diagonal M (grounded caps)'
    alg = [i for i in range(N) if i not in diff]
    Md = M[np.ix_(diff, diff)]
    Kdd = K[np.ix_(diff, diff)]; Kda = K[np.ix_(diff, alg)]; Kad = K[np.ix_(alg, diff)]; Kaa = K[np.ix_(alg, alg)]
    KaaI = np.linalg.inv(Kaa) if alg else np.zeros((0, 0))
    Seff = Kdd - Kda @ KaaI @ Kad
    Minv = np.diag(1.0 / np.diag(Md))
    A = -Minv @ Seff
    nd_ = len(diff)
    # b-map: x_d' = A x_d + Minv (b_d - Kda KaaI b_a)
    Pd = np.zeros((nd_, N))
    Pd[:, diff] = Minv
    if alg:
        Pd[:, alg] = -Minv @ Kda @ KaaI
    # initial DC
    x0 = np.linalg.solve(K, bvec(0.0))
    xd = x0[diff].copy()
    # event times
    bps = sorted({0.0, tstop} | {b for (_, _, w) in bsrc for b in breakpoints(w) if 0 < b < tstop})
    ngrid = int(math.floor(tstop / tstep + 1e-9))
    grid = [k * tstep for k in range(ngrid + 1)]
    events = sorted(set(grid) | set(bps))
    out_t = []; X = []
    t = 0.0
    def record(t, xd):
        b = bvec(t)
        xa = KaaI @ (b[alg] - Kad @ xd) if alg else np.zeros(0)
        x = np.zeros(N); x[diff] = xd; x[alg] = xa
        return x
    gset = set(grid)
    out_t.append(0.0); X.append(record(0.0, xd))
    cache = {}
    Z = np.zeros((nd_, nd_)); I = np.eye(nd_)
    def phis(dt):
        key = round(dt, 27)
        if key not in cache:
            B = np.block([[A, I, Z], [Z, Z, I], [Z, Z, Z]])
            E = expm(B * dt)
            cache[key] = (E[:nd_, :nd_], E[:nd_, nd_:2 * nd_], E[:nd_, 2 * nd_:])
        return cache[key]
    for t1 in events[1:]:
        dt = t1 - t
        if dt <= 1e-3 * tstep * 1e-6:
            t = t1
            continue
        b0 = bvec(t); b1 = bvec(t1)
        s = (b1 - b0) / dt
        E0, F1, F2 = phis(dt)
        xd = E0 @ xd + F1 @ (Pd @ b0) + F2 @ (Pd @ s)
        t = t1
        if t1 in gset:
            out_t.append(t1); X.append(record(t1, xd))
    X = np.array(X)
    names = {}
    for nd, k in nid.items():
        names[f'v({nd})'] = X[:, k]
    for e in br:
        if e['kind'] == 'V' or e['kind'] == 'L':
            names[f'i({e["name"].lower()})'] = X[:, bid[e['name'].lower()]]
    return np.array(out_t), names


# ---------------------------------------------------------------- process helpers
CFG = dict(rustspice='rustspice', ngspice='ngspice', prefix=[])

def _cmd(cmd, timeout):
    return CFG['prefix'] + ['timeout', str(timeout)] + cmd

def run_rs(deck, out, timeout=60, env=None):
    stats = out + '.json'
    for f in (out + '.raw', stats):
        if os.path.exists(f): os.remove(f)
    e = dict(os.environ)
    if env: e.update(env)
    p = subprocess.run(_cmd([CFG['rustspice'], 'run', deck, '--raw', out + '.raw', '--stats', stats], timeout),
                       capture_output=True, text=True, env=e)
    res = dict(rc=p.returncode, stderr=p.stderr[-400:])
    if os.path.exists(stats):
        try: res['stats'] = json.load(open(stats))
        except Exception: res['stats'] = None
    res['raw'] = out + '.raw' if os.path.exists(out + '.raw') else None
    return res

def run_ng(deck, out, timeout=120, extra_cards=()):
    """ngspice -b on a copy of the deck (extra .options / cards inserted before .end); returns the rawfile path."""
    txt = open(deck).read().rstrip('\n').split('\n')
    ends = [k for k, l in enumerate(txt) if l.strip().lower() == '.end']
    i = ends[-1] if ends else len(txt)
    txt = txt[:i] + list(extra_cards) + txt[i:]
    tmp = out + '.deck.sp'
    open(tmp, 'w').write('\n'.join(txt) + '\n')
    if os.path.exists(out + '.raw'): os.remove(out + '.raw')
    p = subprocess.run(_cmd([CFG['ngspice'], '-b', '-r', out + '.raw', tmp], timeout), capture_output=True, text=True)
    return dict(rc=p.returncode, raw=out + '.raw' if os.path.exists(out + '.raw') else None)

def add_common_args(ap):
    ap.add_argument('--rustspice', default=DEFAULT_RUSTSPICE, help='RustSpice CLI (default: $RUSTSPICE, else target/release/rustspice of this repository)')
    ap.add_argument('--ngspice', default=DEFAULT_NGSPICE, help='ngspice executable, black box (default: $NGSPICE, else ngspice on PATH)')
    ap.add_argument('--prefix', default='', help='command prefix for every child, e.g. "taskset -c 0-1 systemd-run --user --scope -q -p MemoryMax=6G" (default: none)')
    ap.add_argument('--workdir', default=None, help='scratch directory (default: a new directory under /dev/shm or the system temp dir)')

def setup(args):
    CFG['rustspice'] = args.rustspice
    CFG['ngspice'] = args.ngspice
    CFG['prefix'] = shlex.split(args.prefix)
    import tempfile
    wd = args.workdir or tempfile.mkdtemp(prefix='noi_', dir='/dev/shm' if os.path.isdir('/dev/shm') else None)
    os.makedirs(wd, exist_ok=True)
    return wd

def dev_vs(ref, other, names=None, floor=1e-6):
    """max over vectors of max|other-ref| / max(swing(ref), floor) (|value| for one-point vectors); inf if other is non-finite; rows aligned by index
    (both outputs of a fixed-step run) else by linear interpolation on the time axis."""
    worst = 0.0; where = None
    t_ref = ref.get('time'); t_oth = other.get('time')
    for k, a in ref.items():
        if k == 'time' or (names is not None and k not in names) or k not in other: continue
        b = other[k]
        if t_ref is not None and t_oth is not None and len(t_ref) != len(t_oth):
            b = np.interp(t_ref, t_oth, b)
        n = min(len(a), len(b))
        if not np.all(np.isfinite(b[:n])):
            return math.inf, k
        scale = float(np.ptp(a)) if len(a) > 1 else abs(float(a[0]))
        d = float(np.max(np.abs(b[:n] - a[:n]))) / max(scale, floor)
        if d > worst: worst, where = d, k
    return worst, where


# ---------------------------------------------------------------- nonlinear controls (card order only)
MOS_MODELS = ['.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)', '.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)']

def cmos_chain_cards(n):
    L = ['VDD vdd 0 DC 3.3', 'VIN in1 0 PULSE(0.0 3.3 0.2n 0.1n 0.1n 2n 4n)']
    for k in range(1, n + 1):
        inp = 'in1' if k == 1 else f'n{k-1}'
        L += [f'MP{k} n{k} {inp} vdd vdd PMOD W=20u L=1u', f'MN{k} n{k} {inp} 0 0 NMOD W=10u L=1u', f'CL{k} n{k} 0 20f']
    return L

def diode_mesh_cards(m):
    L = ['V1 in 0 DC 3', 'Rin in n0_0 100']
    for i in range(m):
        for j in range(m):
            L.append(f'D{i}_{j} n{i}_{j} 0 DMOD')
            if j + 1 < m: L.append(f'Rh{i}_{j} n{i}_{j} n{i}_{j+1} 100')
            if i + 1 < m: L.append(f'Rv{i}_{j} n{i}_{j} n{i+1}_{j} 100')
    return L

def write_text_deck(path, head, cards, tail, title='t'):
    open(path, 'w').write('\n'.join([title] + head + cards + tail + ['.end']) + '\n')

def res_mesh_deck(path, m, shuffle_seed=None):
    L = []
    for i in range(m):
        for j in range(m):
            if j + 1 < m: L.append(f'Rh{i}_{j} n{i}_{j} n{i}_{j+1} 1')
            if i + 1 < m: L.append(f'Rv{i}_{j} n{i}_{j} n{i+1}_{j} 1')
    L += ['Rg n0_0 0 1', f'I1 0 n{m//2}_{m//2} DC 1m']
    if shuffle_seed is not None:
        random.Random(shuffle_seed).shuffle(L)
    write_text_deck(path, [], L, ['.op'], title=f'resistor mesh {m}x{m}')


def selftest():
    """Oracle self-test against closed forms (exit 1 on failure)."""
    import tempfile
    d = tempfile.mkdtemp(prefix='noi_selftest_')
    tau, tr = 1e-9, 100e-12
    els = [dict(name='V1', kind='V', nodes=['in', '0'], tail='PULSE(0 1 0 100p 100p 1 2)'),
           dict(name='R1', kind='R', nodes=['in', 'o'], tail='1k'), dict(name='C1', kind='C', nodes=['o', '0'], tail='1p')]
    p = os.path.join(d, 'rc.sp'); write_deck(p, 't', [], els, ['.tran 10p 5n'])
    t, v = oracle_tran(p)
    ex = np.where(t < tr, (t - tau * (1 - np.exp(-t / tau))) / tr, 1 - (tau / tr) * (np.exp(-(t - tr) / tau) - np.exp(-t / tau)))
    e1 = float(np.max(np.abs(v['v(o)'] - ex)))
    els = [dict(name='V1', kind='V', nodes=['in', '0'], tail='PULSE(0 1 0 1f 1f 1 2)'), dict(name='R1', kind='R', nodes=['in', 'a'], tail='10'),
           dict(name='L1', kind='L', nodes=['a', 'o'], tail='1n'), dict(name='C1', kind='C', nodes=['o', '0'], tail='1p')]
    p = os.path.join(d, 'rlc.sp'); write_deck(p, 't', [], els, ['.tran 1p 2n'])
    t, v = oracle_tran(p)
    R, L, C = 10.0, 1e-9, 1e-12
    a = R / (2 * L); w0 = 1 / math.sqrt(L * C); wd = math.sqrt(w0 ** 2 - a ** 2)
    ex = 1 - np.exp(-a * t) * (np.cos(wd * t) + a / wd * np.sin(wd * t))
    e2 = float(np.max(np.abs(v['v(o)'] - ex)))
    print(f'oracle self-test: RC ramp response error {e1:.2e} (limit 1e-13), series RLC step error {e2:.2e} (limit 3e-5: 1 fs rise time)')
    return 0 if (e1 < 1e-13 and e2 < 3e-5) else 1

if __name__ == '__main__':
    sys.exit(selftest())
