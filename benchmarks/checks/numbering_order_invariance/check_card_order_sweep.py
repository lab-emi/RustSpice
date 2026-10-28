#!/usr/bin/env python3
"""Card-order invariance sweep of RustSpice (exit 1 = at least one violation, exit 2 = tool/setup problem).

Premise: shuffling the CARDS of a deck (node names unchanged) does not change the physics. ngspice is checked for that
premise (shuffled vs natural, black box), RustSpice is checked against
  * an independent exact oracle (matrix exponential, dense f64) for the linear RC / LC ladders,
  * its own natural-order result for the nonlinear controls and the resistor mesh.
A shuffled run is a VIOLATION when RustSpice exits non-zero, writes non-finite values, or deviates more than the
threshold of the family (RC 1e-3 V, LC 5e-3 V on 1 V drives = 50x / 6x the natural-order error; nonlinear 1e-6 of swing).

usage: check_card_order_sweep.py [--rustspice BIN] [--ngspice BIN] [--prefix "taskset -c 0-1"] [--seeds 8] [--quick] [--slow]
(defaults: --rustspice $RUSTSPICE else target/release/rustspice, --ngspice $NGSPICE else ngspice)
"""
import argparse, sys, os, json, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _noi_common import *

ap = argparse.ArgumentParser()
add_common_args(ap)
ap.add_argument('--seeds', type=int, default=8)
ap.add_argument('--quick', action='store_true', help='RC 10/20/40 and LC 10 only, skip the nonlinear and mesh parts')
ap.add_argument('--slow', action='store_true', help='also run the shuffled diode-ladder deck rob_ord_diode_rc_ladder100_cardshuffle_hang under --hang-budget seconds')
ap.add_argument('--hang-budget', type=int, default=60)
ap.add_argument('--corpus', default=str(CASES), help='read-only corpus directory (only used by --slow; default: benchmarks/cases of this repository)')
args = ap.parse_args()
wd = setup(args)
seeds = list(range(1, args.seeds + 1))
rows, violations, premise_violations = [], 0, 0

def fmt(x):
    return 'FAIL' if x is None else ('inf/nan' if not math.isfinite(x) else '%.1e' % x)

def tier(r):
    return ((r.get('stats') or {}).get('solver_kind') or '?')[:4]

# ---------------------------------------------------------------- A. linear ladders against the exact oracle
configs = [('rc', n) for n in (5, 10, 20, 34, 35, 40, 60)] + [('lc', n) for n in (5, 10, 20)]
if args.quick:
    configs = [('rc', 10), ('rc', 20), ('rc', 40), ('lc', 10)]
for kind, n in configs:
    if kind == 'rc':
        base = rc_ladder(n, R='1k', C='1p', src='PULSE(0 1 1n 100p 100p 12n 50n)'); card = '.tran 10p 20n'; thr = 1e-3
    else:
        base = lc_ladder(n, L='1n', C='1p', Rs='50', Rl='50', src='PULSE(0 1 0 100p 100p 1n 10n)'); card = '.tran 2p 2n'; thr = 5e-3
    nat = f'{wd}/{kind}{n}_nat.sp'
    write_deck(nat, f'{kind}{n}', [], base, [card])
    t, orc = oracle_tran(nat)
    ref_cards = sorted(' '.join([e['name']] + e['nodes'] + [e['tail']]) for e in base)
    def err_of(res):
        if res['rc'] != 0 or not res['raw']:
            return None
        raw = read_raw(res['raw'])
        if len(raw['time']) != len(t):
            return None
        worst = 0.0
        for k, ref in orc.items():
            if k.startswith('v(') and k in raw:
                y = raw[k]
                if not np.all(np.isfinite(y)): return math.inf
                worst = max(worst, float(np.max(np.abs(y - ref))))
        return worst
    e_nat = err_of(run_rs(nat, f'{wd}/{kind}{n}_nat'))
    errs, tiers = [], []
    for s in seeds:
        els, _ = permute(base, s, relabel=False)
        assert sorted(' '.join([e['name']] + e['nodes'] + [e['tail']]) for e in els) == ref_cards  # same circuit, order only
        p = f'{wd}/{kind}{n}_s{s}.sp'
        write_deck(p, f'{kind}{n}', [], els, [card])
        r = run_rs(p, f'{wd}/{kind}{n}_s{s}')
        errs.append(err_of(r)); tiers.append(tier(r))
    bad = [s for s, e in zip(seeds, errs) if e is None or not math.isfinite(e) or e > thr]
    # premise: ngspice (as-is, default tolerances) is card-order invariant
    inv = 0.0
    nrn = run_ng(nat, f'{wd}/{kind}{n}_ng_nat')
    for s in seeds[:2]:
        nrs = run_ng(f'{wd}/{kind}{n}_s{s}.sp', f'{wd}/{kind}{n}_ng_s{s}')
        if nrn['raw'] and nrs['raw']:
            d, _ = dev_vs(read_raw(nrn['raw']), read_raw(nrs['raw']))
            inv = max(inv, d)
        else:
            inv = math.inf
    if inv > 1e-6: premise_violations += 1
    violations += len(bad)
    rows.append(dict(family=f'{kind}{n}', unknowns=n + 2 if kind == 'rc' else 2 * n + 3, natural_err=e_nat, shuffled_err=errs, tiers=tiers,
                     bad_seeds=bad, threshold=thr, ngspice_invariance=inv))
    print(f'{kind.upper()}{n:<3d} natural {fmt(e_nat):>8}  shuffled bad {len(bad)}/{len(seeds)}  ngspice dev {inv:.1e}  '
          + ' '.join(f's{s}:{fmt(e)}:{tr}' for s, e, tr in zip(seeds, errs, tiers)), flush=True)

# ---------------------------------------------------------------- B. nonlinear controls and mesh: shuffled vs natural RustSpice
if not args.quick:
    nl = [('cmos5_tran', MOS_MODELS, cmos_chain_cards(5), ['.tran 1p 3n']),
          ('cmos8_tran', MOS_MODELS, cmos_chain_cards(8), ['.tran 1p 3.5n']),
          ('diode_mesh8_dc', ['.model DMOD D(IS=1e-14 N=1.0)'], diode_mesh_cards(8), ['.dc V1 0 3 0.1']),
          ('diode_mesh16_dc', ['.model DMOD D(IS=1e-14 N=1.0)'], diode_mesh_cards(16), ['.dc V1 0 3 0.1']),
          ('res_mesh20_op', None, None, None)]
    for name, head, cards, tail in nl:
        nat = f'{wd}/{name}_nat.sp'
        if head is None:
            res_mesh_deck(nat, 20)
        else:
            write_text_deck(nat, head, cards, tail)
        rn = run_rs(nat, f'{wd}/{name}_nat')
        if rn['rc'] != 0 or not rn['raw']:
            print(f'{name}: natural order itself fails ({rn["stderr"].strip()[:80]}) -> not a card-order question, skipped')
            continue
        ref = read_raw(rn['raw'])
        devs, tiers, bad = [], [], []
        for s in seeds:
            p = f'{wd}/{name}_s{s}.sp'
            if head is None:
                res_mesh_deck(p, 20, shuffle_seed=s)
            else:
                c = list(cards); random.Random(s).shuffle(c)
                write_text_deck(p, head, c, tail)
            r = run_rs(p, f'{wd}/{name}_s{s}')
            tiers.append(tier(r))
            if r['rc'] != 0 or not r['raw']:
                devs.append(None); bad.append(s); continue
            d, _ = dev_vs(ref, read_raw(r['raw']))
            devs.append(d)
            if d > 1e-6: bad.append(s)
        inv = 0.0
        ngn = run_ng(nat, f'{wd}/{name}_ng_nat')
        for s in seeds[:2]:
            ngs = run_ng(f'{wd}/{name}_s{s}.sp', f'{wd}/{name}_ng_s{s}')
            if ngn['raw'] and ngs['raw']:
                a, b = read_raw(ngn['raw']), read_raw(ngs['raw'])
                if 'time' in a and len(a['time']) != len(b['time']):
                    d = math.nan
                else:
                    d, _ = dev_vs(a, b)
                inv = max(inv, d) if math.isfinite(d) else inv
        violations += len(bad)
        rows.append(dict(family=name, shuffled_dev=devs, tiers=tiers, bad_seeds=bad, ngspice_invariance=inv))
        print(f'{name:<16s} shuffled-vs-natural bad {len(bad)}/{len(seeds)}  ngspice dev {inv:.1e}  '
              + ' '.join(f's{s}:{fmt(e)}:{tr}' for s, e, tr in zip(seeds, devs, tiers)), flush=True)

# ---------------------------------------------------------------- C. runtime cliff of the shuffled diode ladder
if args.slow:
    nat = f'{args.corpus}/performance/perf_med_diode_rc_ladder_tran_100.sp'
    shuf = f'{args.corpus}/robustness/rob_ord_diode_rc_ladder100_cardshuffle_hang.sp'
    t0 = time.time(); rn = run_rs(nat, f'{wd}/dl_nat'); tn = time.time() - t0
    t0 = time.time(); rs = run_rs(shuf, f'{wd}/dl_shuf', timeout=args.hang_budget); ts = time.time() - t0
    st = rs.get('stats') or {}
    over = rs['rc'] == 124
    violations += 1 if over or ts > 20 * max(tn, 0.05) else 0
    rows.append(dict(family='diode_ladder100', natural_s=tn, shuffled_s=ts, shuffled_rc=rs['rc'], shuffled_points=st.get('points')))
    print(f'diode ladder 100: natural {tn:.2f} s (rc {rn["rc"]}), shuffled {ts:.1f} s (rc {rs["rc"]}{" = killed at the budget" if over else ""}) points {st.get("points")}')

json.dump(dict(rows=rows, violations=violations, premise_violations=premise_violations), open(f'{wd}/card_order_sweep.json', 'w'), default=str)
print(f'\nscratch: {wd}')
if premise_violations:
    print(f'PREMISE VIOLATED for {premise_violations} families: ngspice itself depends on card order (> 1e-6 of swing); results above are not interpretable')
    sys.exit(2)
print(f'VIOLATIONS: {violations} shuffled runs (RustSpice failed, wrote non-finite values or deviated beyond the threshold)')
sys.exit(1 if violations else 0)
