#!/usr/bin/env python3
"""Path-independence of the .dc decks of cap_xm_* (a property the differential engine cannot
express because it compares one sweep against one reference sweep):

  (1) every point of the sweep must equal a stand-alone `.op` run with the swept source set to that value (fresh Newton
      start from zero instead of continuation from the previous point),
  (2) the sweep run backwards (`.dc SRC stop start -step`) must reproduce the forward sweep point by point (no hysteresis
      from stale Newton state; the circuits have a unique solution).

Checked in RustSpice and in ngspice (black box, tight options injected so that its own convergence slack does not mask
anything), threshold = the deck's own tolerance band (tol.abs + tol.rel*|value|) for both tools.
Exit 0 = all comparisons inside the band, 1 = violation, 2 = tool missing.
Environment: RUSTSPICE, NGSPICE, EAMH_CPUS (taskset list, default: no pinning).  Runs ~5 decks x ~25 points x 2 tools, a few MiB each.
"""
import os, re, shutil, subprocess, sys, tempfile
import numpy as np
HERE = os.path.dirname(os.path.abspath(__file__))
# <repo>/modules/RustSpice/benchmarks/checks/element_analysis_matrix_holes/<script> -> parents[3] = <repo>/modules/RustSpice
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..'))
CASES = os.path.join(ROOT, 'benchmarks', 'cases', 'capability')
RS = os.environ.get('RUSTSPICE', os.path.join(ROOT, 'target', 'release', 'rustspice')); NG = os.environ.get('NGSPICE', 'ngspice'); CPUS = os.environ.get('EAMH_CPUS', '')
PREFIX = 'cap_xm_'
DECKS = ['l_dc_sweep_diode', 'l_dc_sweep_isrc_diode', 'g_dc_sweep_diode_load', 'sub_dc_sweep_diode_divider', 'sub_g_internal_ctrl_dc_sweep']
NG_TIGHT = 'reltol=1e-9 vntol=1e-12 abstol=1e-15'

def read_raw(path):
    data = open(path, 'rb').read(); i = data.find(b'Binary:\n'); hdr = data[:i].decode('latin1').splitlines(); names = []; inv = False
    for ln in hdr:
        if ln.startswith('No. Variables:'): nvar = int(ln.split(':')[1])
        elif ln.startswith('No. Points:'): npts = int(ln.split(':')[1])
        elif ln.startswith('Variables:'): inv = True
        elif inv and ln.strip(): names.append(ln.split()[1].lower())
    arr = np.frombuffer(data, dtype='<f8', count=nvar * npts, offset=i + 8).reshape(npts, nvar)
    return {'x': arr[:, 0].copy(), 'v': {n: arr[:, k].copy() for k, n in enumerate(names)}}

def run(tool, text, work, tag):
    d = os.path.join(work, tag + '.sp'); open(d, 'w').write(text); raw = os.path.join(work, tag + '.raw')
    cmd = [RS, 'run', d, '--raw', raw] if tool == 'rs' else [NG, '-b', '-r', raw, d]
    p = subprocess.run((['taskset', '-c', CPUS] if CPUS else []) + cmd, capture_output=True, text=True, timeout=120)
    if p.returncode != 0 or not os.path.exists(raw): raise RuntimeError('%s failed rc=%d' % (tool, p.returncode))
    return read_raw(raw)

def num(tok):
    s = {'meg': 1e6, 'k': 1e3, 'm': 1e-3, 'u': 1e-6, 'n': 1e-9, 'p': 1e-12}
    mm = re.match(r'^([-+]?[0-9.]+(?:e[-+]?[0-9]+)?)(meg|[ktmunp])?', tok.lower()); return float(mm.group(1)) * (s[mm.group(2)] if mm.group(2) else 1.0)

def main():
    for t in (RS, NG):
        if not (os.path.isfile(t) or shutil.which(t)): print('missing tool', t); return 2
    work = tempfile.mkdtemp(prefix='eamh_sweep_'); bad = 0
    try:
        for stem in DECKS:
            text = open(os.path.join(CASES, PREFIX + stem + '.sp')).read()
            md = {}
            for ln in text.splitlines()[1:]:
                if ln.startswith('* bench.'): k, _, v = ln[2:].partition(':'); md[k.strip()[6:]] = v.strip()
            probes = [p.strip() for p in md['probes'].split(',')]; tabs = num(md.get('tol.abs', '1e-9')); trel = float(md.get('tol.rel', '1e-6'))
            m = re.search(r'(?im)^\.dc\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)', text); src, a, b, st = m.group(1), num(m.group(2)), num(m.group(3)), num(m.group(4))
            card = m.group(0)
            for tool in ('rs', 'ng'):
                base = text if tool == 'rs' else re.sub(r'(?im)^\.end\s*$', '.options %s\n.end' % NG_TIGHT, text, count=1)
                fwd = run(tool, base, work, 'f'); bwd = run(tool, re.sub(r'(?im)^\.dc\s.*$', '.dc %s %.12g %.12g %.12g' % (src, b, a, -st), base, count=1), work, 'b')
                # (2) backward vs forward: align on the sweep value
                xb = bwd['x'][::-1]; worst2 = 0.0
                assert len(xb) == len(fwd['x']) and np.allclose(xb, fwd['x'], atol=1e-9 * max(abs(a), abs(b), 1e-12)), 'grid mismatch'
                for p in probes:
                    ref = fwd['v'][p]; band = tabs + trel * np.abs(ref)
                    worst2 = max(worst2, np.max(np.abs(bwd['v'][p][::-1] - ref) / band))
                # (1) every point vs a stand-alone .op
                worst1 = 0.0; npts = len(fwd['x'])
                for k in range(npts):
                    val = fwd['x'][k]
                    # the source card `V1 a 0 0` / `I1 0 a 0` carries the DC value in its last token
                    t1 = re.sub(r'(?im)^(%s\s+\S+\s+\S+\s+)\S+\s*$' % re.escape(src), lambda mm: '%s%.17g' % (mm.group(1), val), base, count=1)
                    t1 = re.sub(r'(?im)^\.dc\s.*$', '.op', t1, count=1)
                    r1 = run(tool, t1, work, 'op')
                    for p in probes:
                        ref = fwd['v'][p][k]; band = tabs + trel * abs(ref)
                        worst1 = max(worst1, abs(r1['v'][p][0] - ref) / band)
                ok = worst1 <= 1.0 and worst2 <= 1.0; bad += (not ok)
                print('%-4s %-38s %-2s %2d points: sweep-vs-.op worst %.3g and backward-vs-forward worst %.3g (fractions of the deck band; must be <= 1)' % ('OK' if ok else 'FAIL', stem, tool, npts, worst1, worst2))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    print('RESULT: %s (%d violation(s))' % ('PASS' if bad == 0 else 'FAIL', bad)); return 0 if bad == 0 else 1

if __name__ == '__main__':
    sys.exit(main())
