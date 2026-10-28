#!/usr/bin/env python3
"""Self-consistency check without any external reference (exit 1 = violation, exit 2 = tool problem).

ONE deck holds the same 30-stage RC ladder twice on one source: block A (a1..a30) in natural card order, block B (b1..b30)
with its cards shuffled. Physically v(aK) == v(bK) for every K and every time point, so a simulator that does not depend on
card order returns |v(aK) - v(bK)| at round-off level. The script builds the deck for seeds 1..N (block B shuffled by
random.Random(seed)), runs RustSpice and ngspice (black box, default tolerances) and reports
    max over K and t of |v(aK) - v(bK)|
for each. Violation: RustSpice > 1e-6 V (1 V drive; ngspice measures 2e-20), non-finite values or a failed run.
The checked-in deck rob_ord_twin_rcladder30_blocks.sp (seed 3) is checked as well.

usage: check_twin_blocks.py [--rustspice BIN] [--ngspice BIN] [--prefix "taskset -c 0-1"] [--seeds 8]
(defaults: --rustspice $RUSTSPICE else target/release/rustspice, --ngspice $NGSPICE else ngspice)
"""
import argparse, sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _noi_common import *

ap = argparse.ArgumentParser()
add_common_args(ap)
ap.add_argument('--seeds', type=int, default=8)
ap.add_argument('--threshold', type=float, default=1e-6)
args = ap.parse_args()
wd = setup(args)
N = 30

def twin_deck(path, seed):
    A, B = [], []
    for k in range(1, N + 1):
        pa = 'in' if k == 1 else f'a{k-1}'; pb = 'in' if k == 1 else f'b{k-1}'
        A += [dict(name=f'RA{k}', kind='R', nodes=[pa, f'a{k}'], tail='1k'), dict(name=f'CA{k}', kind='C', nodes=[f'a{k}', '0'], tail='1p')]
        B += [dict(name=f'RB{k}', kind='R', nodes=[pb, f'b{k}'], tail='1k'), dict(name=f'CB{k}', kind='C', nodes=[f'b{k}', '0'], tail='1p')]
    B, _ = permute(B, seed, relabel=False)
    els = [dict(name='V1', kind='V', nodes=['in', '0'], tail='PULSE(0 1 1n 100p 100p 12n 50n)')] + A + B
    write_deck(path, f'twin RC ladders, block B cards shuffled (seed {seed})', [], els, ['.tran 10p 20n'])

def twin_gap(raw_path):
    raw = read_raw(raw_path)
    worst = 0.0
    for k in range(1, N + 1):
        a, b = raw[f'v(a{k})'], raw[f'v(b{k})']
        if not (np.all(np.isfinite(a)) and np.all(np.isfinite(b))):
            return math.inf
        worst = max(worst, float(np.max(np.abs(a - b))))
    return worst

jobs = [(f's{s}', None, s) for s in range(1, args.seeds + 1)]
jobs.append(('checked_in_deck', str(CASES / 'robustness' / 'rob_ord_twin_rcladder30_blocks.sp'), None))
bad = 0
rows = []
for name, deck, seed in jobs:
    if deck is None:
        deck = f'{wd}/twin_{name}.sp'; twin_deck(deck, seed)
    elif not os.path.exists(deck):
        print(f'{name}: deck not found ({deck}), skipped'); continue
    r = run_rs(deck, f'{wd}/rs_{name}')
    n = run_ng(deck, f'{wd}/ng_{name}')
    g_rs = twin_gap(r['raw']) if r['rc'] == 0 and r['raw'] else None
    g_ng = twin_gap(n['raw']) if n['raw'] else None
    ok = g_rs is not None and math.isfinite(g_rs) and g_rs <= args.threshold
    bad += 0 if ok else 1
    rows.append(dict(case=name, rustspice_gap=g_rs, ngspice_gap=g_ng, tier=(r.get('stats') or {}).get('solver_kind')))
    f = lambda x: 'FAIL' if x is None else ('inf/nan' if not math.isfinite(x) else '%.1e' % x)
    print(f'{name:<16s} max|vA-vB|  RustSpice {f(g_rs):>8} ({(r.get("stats") or {}).get("solver_kind", "?")[:6]})   ngspice {f(g_ng):>8}   {"ok" if ok else "VIOLATION"}', flush=True)
json.dump(rows, open(f'{wd}/twin_blocks.json', 'w'), default=str)
print(f'\nscratch: {wd}\nVIOLATIONS: {bad} of {len(rows)} decks (threshold {args.threshold:g} V)')
sys.exit(1 if bad else 0)
