#!/usr/bin/env python3
"""Hierarchy invariance of the cap_xm_* subcircuit decks (a self-consistency property the
differential engine cannot express): a deck written with .subckt/X cards must give the SAME numbers as the hand-flattened
netlist of identical elements, in RustSpice and in ngspice (black-box executable).

For every hierarchical deck a flat twin (explicit element cards, internal nodes spelled exactly like the flattened
names, e.g. `x1.k`) is written to a temporary directory; every v() vector of the hierarchical result must equal the flat
twin's vector of the same name to 1e-9 of that vector's swing, in both tools.  The flat twin of the `instance_name`
deck names the source plainly (no `pulse` in any element name), so for that deck the check ALSO documents the gap:
RustSpice hierarchical != flat on the failing probe, ngspice hierarchical == flat.

Exit 0 = every comparison within threshold (and the documented gap present), 1 = violation, 2 = tool missing.
Environment: RUSTSPICE, NGSPICE, EAMH_CPUS (taskset list, default: no pinning).  Memory: a few MiB.
"""
import os, re, shutil, subprocess, sys, tempfile
import numpy as np
HERE = os.path.dirname(os.path.abspath(__file__))
# <repo>/modules/RustSpice/benchmarks/checks/element_analysis_matrix_holes/<script> -> parents[3] = <repo>/modules/RustSpice
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..'))
CASES = os.path.join(ROOT, 'benchmarks', 'cases', 'capability')
RS = os.environ.get('RUSTSPICE', os.path.join(ROOT, 'target', 'release', 'rustspice')); NG = os.environ.get('NGSPICE', 'ngspice')
CPUS = os.environ.get('EAMH_CPUS', ''); PREFIX = 'cap_xm_'
THRESH = 1e-9

def read_raw(path):
    data = open(path, 'rb').read(); pos = 0; last = None
    while pos < len(data):
        i = data.find(b'Binary:\n', pos)
        if i < 0: break
        hdr = data[pos:i].decode('latin1').splitlines(); pos = i + 8; names = []; inv = False
        for ln in hdr:
            if ln.startswith('No. Variables:'): nvar = int(ln.split(':')[1])
            elif ln.startswith('No. Points:'): npts = int(ln.split(':')[1])
            elif ln.startswith('Variables:'): inv = True
            elif inv and ln.strip(): names.append(ln.split()[1].lower())
        arr = np.frombuffer(data, dtype='<f8', count=nvar * npts, offset=pos).reshape(npts, nvar); pos += 8 * nvar * npts
        last = {'x': arr[:, 0].copy(), 'v': {n: arr[:, k].copy() for k, n in enumerate(names)}}
    return last

def pin(c): return (['taskset', '-c', CPUS] if CPUS else []) + c
def run(tool, deck, work, tag):
    raw = os.path.join(work, tag + '.raw')
    cmd = [RS, 'run', deck, '--raw', raw] if tool == 'rs' else [NG, '-b', '-r', raw, deck]
    p = subprocess.run(pin(cmd), capture_output=True, text=True, timeout=300)
    if p.returncode != 0 or not os.path.exists(raw): raise RuntimeError('%s failed on %s rc=%d' % (tool, deck, p.returncode))
    return read_raw(raw)

def card(text, pat):
    return re.search(r'(?im)^%s.*$' % pat, text).group(0)

FLAT = {
 'sub_dc_sweep_diode_divider': """flat twin
V1 in 0 0
R1_x1 in x1.k 2k
D1_x1 x1.k m dm
R1_x2 m x2.k 2k
D1_x2 x2.k o dm
R2 o 0 1k
.model dm D(IS=1e-14 N=1)
%(ana)s
.end
""",
 'sub_g_internal_ctrl_dc_sweep': """flat twin
.model dm D(IS=1e-14 N=1)
V1 in 0 0
R1_x1 in x1.m 1k
R2_x1 x1.m 0 1k
G1_x1 0 n1 x1.m 0 4m
R3_x1 n1 0 1k
R1_x2 n1 x2.m 1k
R2_x2 x2.m 0 1k
G1_x2 0 n2 x2.m 0 4m
R3_x2 n2 0 1k
R1_x3 n2 x3.m 1k
R2_x3 x3.m 0 1k
G1_x3 0 n3 x3.m 0 4m
R3_x3 n3 0 1k
D1 n3 0 dm
R4 n3 0 2k
%(ana)s
.end
""",
 'sub_cmos_chain4_tran': """flat twin
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
VDD vdd 0 DC 3.3
VIN in1 0 PULSE(0.0 3.3 0.2n 0.1n 0.1n 2n 4n)
MP1_x1 n1 in1 vdd vdd PMOD W=20u L=1u
MN1_x1 n1 in1 0 0 NMOD W=10u L=1u
CL_x1 n1 0 20f
MP1_x2 n2 n1 vdd vdd PMOD W=20u L=1u
MN1_x2 n2 n1 0 0 NMOD W=10u L=1u
CL_x2 n2 0 20f
MP1_x3 n3 n2 vdd vdd PMOD W=20u L=1u
MN1_x3 n3 n2 0 0 NMOD W=10u L=1u
CL_x3 n3 0 20f
MP1_x4 n4 n3 vdd vdd PMOD W=20u L=1u
MN1_x4 n4 n3 0 0 NMOD W=10u L=1u
CL_x4 n4 0 20f
%(ana)s
.end
""",
 'sub_rlc_ladder_tran': """flat twin
V1 in 0 PULSE(0 1 5n 2n 2n 40n 100n)
R1_x1x1 in x1.x1.m 5
L1_x1x1 x1.x1.m n1 100n
C1_x1 n1 0 100p
R1_x2x1 n1 x2.x1.m 5
L1_x2x1 x2.x1.m n2 100n
C1_x2 n2 0 100p
R1_x3x1 n2 x3.x1.m 5
L1_x3x1 x3.x1.m n3 100n
C1_x3 n3 0 100p
RL n3 0 50
%(ana)s
.end
""",
 'sub_source_pulse_sin_tran': """flat twin
V1_x1 a 0 PULSE(0 2 10n 5n 5n 40n 100n)
I1_x1 0 b SIN(0 1m 10meg 0)
R1 a out 1k
R2 b out 2k
R3 out 0 1k
C1 out 0 20p
C2 b 0 5p
%(ana)s
.end
""",
 'sub_instance_name_waveform_tran': """flat twin
I1_a 0 o1 SIN(0 1m 10meg 0)
R1_a o1 0 1k
I1_b 0 o2 SIN(0 1m 10meg 0)
R1_b o2 0 1k
%(ana)s
.end
""",
}
GAP = {'sub_instance_name_waveform_tran'}

def main():
    for t in (RS, NG):
        if not (os.path.isfile(t) or shutil.which(t)): print('missing tool', t); return 2
    work = tempfile.mkdtemp(prefix='eamh_hier_'); bad = 0
    try:
        for stem, tmpl in FLAT.items():
            hier = os.path.join(CASES, PREFIX + stem + '.sp'); text = open(hier).read()
            ana = card(text, r'\.(dc|tran)\s')
            flat = os.path.join(work, stem + '_flat.sp'); open(flat, 'w').write(tmpl % {'ana': ana})
            for tool in ('rs', 'ng'):
                h = run(tool, hier, work, tool + '_h'); f = run(tool, flat, work, tool + '_f')
                common = [n for n in h['v'] if n.startswith('v(') and n in f['v']]
                nhier = [n for n in h['v'] if n.startswith('v(')]
                if len(h['x']) != len(f['x']) or not np.allclose(h['x'], f['x'], rtol=1e-12, atol=0):
                    print('FAIL %-38s %s sweep/time axes differ (%d vs %d points)' % (stem, tool, len(h['x']), len(f['x']))); bad += 1; continue
                worst = 0.0; wn = ''
                for n in common:
                    sw = max(np.ptp(f['v'][n]), 1e-9); d = np.max(np.abs(h['v'][n] - f['v'][n])) / sw
                    if d > worst: worst, wn = d, n
                missing = [n for n in nhier if n not in f['v']]
                is_gap = stem in GAP and tool == 'rs'
                ok = (worst > 1e-3) if is_gap else (worst <= THRESH)
                if not is_gap and missing: ok = False
                bad += (not ok)
                print('%-4s %-38s %-2s hier-vs-flat max %.2e of swing (worst vector %s), %d vectors compared%s%s' % ('OK' if ok else 'FAIL', stem, tool, worst, wn, len(common), ', MISSING in flat: %s' % missing if missing else '', '  [documented gap: RustSpice hier != flat]' if is_gap else ''))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    print('RESULT: %s (%d violation(s))' % ('PASS' if bad == 0 else 'FAIL', bad)); return 0 if bad == 0 else 1

if __name__ == '__main__':
    sys.exit(main())
