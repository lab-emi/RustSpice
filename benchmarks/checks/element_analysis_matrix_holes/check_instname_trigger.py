#!/usr/bin/env python3
"""Mechanism check for the known-gap deck cap_xm_sub_instance_name_waveform_tran.

Claim documented in the deck: RustSpice flattens a subcircuit instance into element names such as `i1_xpulse1`, then looks
for the waveform keyword as a substring of the WHOLE card text, name included (PULSE is searched before SIN).  Therefore
a legal SIN source inside an instance whose name contains `pulse` is read as PULSE(<the SIN arguments>), whereas a PULSE
source inside an instance whose name contains `sin` is read correctly, and renaming the instance flips the result.

Oracle: closed forms  v(o) = 1 kOhm * 1 mA * sin(2 pi 10 MHz t)  (SIN case)  and the exact PULSE waveform (PULSE case),
both algebraic (source into a resistor).  ngspice (black box) is run on every variant as a cross-check of the oracle.
Exit 0 = RustSpice wrong exactly on the variants predicted by the substring mechanism and right on all others, ngspice
right everywhere; 1 = prediction violated; 2 = tool missing.
Environment: RUSTSPICE, NGSPICE, EAMH_CPUS (taskset list, default: no pinning).
"""
import os, re, shutil, subprocess, sys, tempfile
import numpy as np
# <repo>/modules/RustSpice/benchmarks/checks/element_analysis_matrix_holes/<script> -> three levels up = <repo>/modules/RustSpice
ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
RS = os.environ.get('RUSTSPICE', os.path.join(ROOT, 'target', 'release', 'rustspice')); NG = os.environ.get('NGSPICE', 'ngspice'); CPUS = os.environ.get('EAMH_CPUS', '')

def read_raw(path):
    data = open(path, 'rb').read(); i = data.find(b'Binary:\n'); hdr = data[:i].decode('latin1').splitlines(); names = []; inv = False
    for ln in hdr:
        if ln.startswith('No. Variables:'): nvar = int(ln.split(':')[1])
        elif ln.startswith('No. Points:'): npts = int(ln.split(':')[1])
        elif ln.startswith('Variables:'): inv = True
        elif inv and ln.strip(): names.append(ln.split()[1].lower())
    arr = np.frombuffer(data, dtype='<f8', count=nvar * npts, offset=i + 8).reshape(npts, nvar)
    return {n: arr[:, k] for k, n in enumerate(names)}

def run(tool, deck, work, tag):
    raw = os.path.join(work, tag + '.raw'); cmd = [RS, 'run', deck, '--raw', raw] if tool == 'rs' else [NG, '-b', '-r', raw, deck]
    p = subprocess.run((['taskset', '-c', CPUS] if CPUS else []) + cmd, capture_output=True, text=True, timeout=120)
    if p.returncode != 0 or not os.path.exists(raw): raise RuntimeError('%s failed rc=%d' % (tool, p.returncode))
    return read_raw(raw)

SIN_DECK = """variant
.subckt gen out
I1 0 out SIN(0 1m 10meg 0)
R1 out 0 1k
.ends
%s o gen
.tran 1n 200n
.end
"""
PULSE_DECK = """variant
.subckt gen out
V1 out 0 PULSE(0 2 10n 5n 5n 40n 100n)
R1 out 0 1k
.ends
%s o gen
.tran 1n 200n
.end
"""
def pulse(t, v1=0, v2=2, td=10e-9, tr=5e-9, tf=5e-9, pw=40e-9, per=100e-9):
    if t < td: return v1
    tt = (t - td) % per
    if tt < tr: return v1 + (v2 - v1) * tt / tr
    if tt < tr + pw: return v2
    if tt < tr + pw + tf: return v2 + (v1 - v2) * (tt - tr - pw) / tf
    return v1

# (deck template, instance card name, source kind, RustSpice expected wrong?)
VARIANTS = [(SIN_DECK, 'X1', 'sin', False), (SIN_DECK, 'Xpuls1', 'sin', False), (SIN_DECK, 'Xsin1', 'sin', False),
            (SIN_DECK, 'Xpulse1', 'sin', True), (SIN_DECK, 'XPULSE_gen', 'sin', True), (SIN_DECK, 'Xmypulsegen', 'sin', True),
            (PULSE_DECK, 'X1', 'pulse', False), (PULSE_DECK, 'Xsin1', 'pulse', False), (PULSE_DECK, 'Xpulse1', 'pulse', False)]

def main():
    for t in (RS, NG):
        if not (os.path.isfile(t) or shutil.which(t)): print('missing tool', t); return 2
    work = tempfile.mkdtemp(prefix='eamh_inst_'); bad = 0
    try:
        for k, (tmpl, inst, kind, rs_wrong) in enumerate(VARIANTS):
            d = os.path.join(work, 'v%d.sp' % k); open(d, 'w').write(tmpl % inst)
            for tool in ('rs', 'ng'):
                r = run(tool, d, work, '%s%d' % (tool, k)); t = r['time']; v = r['v(o)']
                orc = np.sin(2 * np.pi * 10e6 * t) if kind == 'sin' else np.array([pulse(x) for x in t])
                err = np.max(np.abs(v - orc)) / np.ptp(orc)
                wrong = err > 5e-3
                expect_wrong = rs_wrong if tool == 'rs' else False
                ok = wrong == expect_wrong; bad += (not ok)
                print('%-4s %-8s %-6s source=%-5s instance card %-12s error vs closed form %.3e (%s, expected %s)' % ('OK' if ok else 'FAIL', tool, 'wrong' if wrong else 'right', kind, inst, err, 'wrong' if wrong else 'right', 'wrong' if expect_wrong else 'right'))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    print('RESULT: %s (%d violation(s))' % ('PASS' if bad == 0 else 'FAIL', bad)); return 0 if bad == 0 else 1

if __name__ == '__main__':
    sys.exit(main())
