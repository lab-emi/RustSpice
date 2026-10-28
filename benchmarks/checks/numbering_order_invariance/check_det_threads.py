#!/usr/bin/env python3
"""Determinism regression anchor (exit 1 = a rawfile differs between configurations, exit 2 = tool problem).

The same deck is run repeatedly with RustSpice under different thread/affinity configurations and the SHA-256 of the whole
binary rawfile (its header carries `Date: n/a`, so it is reproducible) must be identical in all of them:
  default / RAYON_NUM_THREADS=1 / =4 / =32 / OMP_NUM_THREADS=8 / repeated runs, on 1 and on 2 cores of the allowed set.
Decks (generated here): RC ladder 100 (tran, natural order), resistor mesh 20 (op, 400 unknowns), 5-stage CMOS chain (tran),
diode mesh 16 (dc sweep), resistor mesh 60 (op, 3601 unknowns, faer tier); --big adds mesh 100 (10001 unknowns, ~0.7 GB RSS).
Core sets are taken from the CURRENT affinity (run the script under `taskset -c ...`); configurations on 8 and 16 cores are only
run with --wide (they need an otherwise idle host).

usage: taskset -c 0-7 check_det_threads.py [--rustspice BIN] [--prefix "systemd-run --user --scope -q -p MemoryMax=6G"] [--big] [--wide]
"""
import argparse, sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _noi_common import *

ap = argparse.ArgumentParser()
add_common_args(ap)
ap.add_argument('--big', action='store_true')
ap.add_argument('--wide', action='store_true')
args = ap.parse_args()
wd = setup(args)
aff = sorted(os.sched_getaffinity(0))
if len(aff) < 2:
    print('need an affinity set of at least 2 cores', file=sys.stderr); sys.exit(2)
def cores(k): return ','.join(map(str, aff[:min(k, len(aff))]))

decks = {}
p = f'{wd}/rc100.sp'; write_deck(p, 'rc100', [], rc_ladder(100, R='1k', C='1p', src='PULSE(0 1 1n 100p 100p 12n 50n)'), ['.tran 10p 20n']); decks['rc_ladder100_tran'] = p
p = f'{wd}/mesh20.sp'; res_mesh_deck(p, 20); decks['res_mesh20_op'] = p
p = f'{wd}/cmos5.sp'; write_text_deck(p, MOS_MODELS, cmos_chain_cards(5), ['.tran 1p 3n']); decks['cmos5_tran'] = p
p = f'{wd}/dmesh16.sp'; write_text_deck(p, ['.model DMOD D(IS=1e-14 N=1.0)'], diode_mesh_cards(16), ['.dc V1 0 3 0.1']); decks['diode_mesh16_dc'] = p
p = f'{wd}/mesh60.sp'; res_mesh_deck(p, 60); decks['res_mesh60_op'] = p
if args.big:
    p = f'{wd}/mesh100.sp'; res_mesh_deck(p, 100); decks['res_mesh100_op'] = p

R, O = 'RAYON_NUM_THREADS', 'OMP_NUM_THREADS'
configs = [('default_1core', {}, 1), ('default_1core_again', {}, 1), ('rayon1_1core', {R: '1'}, 1), ('rayon4_1core', {R: '4'}, 1),
           ('default_2core', {}, 2), ('rayon32_2core', {R: '32'}, 2), ('omp8_2core', {O: '8'}, 2), ('rayon2_omp8_2core', {R: '2', O: '8'}, 2)]
if args.wide:
    configs += [('default_8core', {}, 8), ('rayon32_8core', {R: '32'}, 8), ('rayon32_16core', {R: '32', O: '8'}, 16)]

def sha(path):
    return hashlib.sha256(open(path, 'rb').read()).hexdigest()

bad = 0
for dn, deck in decks.items():
    hashes, rss, info = {}, 0, ''
    for cn, env, nc in configs:
        out = f'{wd}/{dn}_{cn}'
        for f in (out + '.raw', out + '.json'):
            if os.path.exists(f): os.remove(f)
        e = dict(os.environ); e.update(env)
        cmd = CFG['prefix'] + ['timeout', '170', 'taskset', '-c', cores(nc), CFG['rustspice'], 'run', deck, '--raw', out + '.raw', '--stats', out + '.json']
        pr = subprocess.run(cmd, capture_output=True, text=True, env=e)
        if pr.returncode != 0 or not os.path.exists(out + '.raw'):
            hashes[cn] = f'rc={pr.returncode}'; continue
        hashes[cn] = sha(out + '.raw')
        st = json.load(open(out + '.json'))
        rss = max(rss, st.get('peak_rss_kib') or 0); info = f"{st.get('solver_kind')}, {st.get('points')} rows"
    ref = hashes[configs[0][0]]
    ok = all(h == ref for h in hashes.values()) and not ref.startswith('rc=')
    bad += 0 if ok else 1
    print(f'{dn:<20s} {len(hashes)} configs  {"identical sha256 " + ref[:16] if ok else "MISMATCH " + json.dumps({k: v[:12] for k, v in hashes.items()})}  [{info}, peak RSS {rss/1024:.0f} MiB]', flush=True)
print(f'\nscratch: {wd}\nMISMATCHES: {bad} of {len(decks)} decks')
sys.exit(1 if bad else 0)
