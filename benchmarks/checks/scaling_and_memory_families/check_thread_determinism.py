#!/usr/bin/env python3
"""Core-count / thread-count axis for `rustspice run` (SUITE_SPEC section 6: self-consistency the differential engine cannot express).

Checks, per deck (a 2-D mesh DC, a 3-D mesh DC, an RC-mesh transient, a diode-clamp ladder transient, all generated here):
  1. DETERMINISM (exit 1 on violation): the binary rawfile written by `rustspice run --raw` is byte-identical for
       - every CPU affinity set given with --cpus (default: one core, and two cores),
       - RAYON_NUM_THREADS in {unset, 1, 2, 4} and OMP_NUM_THREADS in {unset, 1, 4},
       - a repeated run (same configuration twice).
  2. THREAD USE (information): the maximum number of OS threads (/proc/<pid>/status `Threads:`) seen while `run` executes, the wall
     time and the CPU time (rusage user+system) per configuration. cpu/wall <= 1.1 means no parallel CPU use whatever the number of
     threads that exist; a wall-time difference between affinity sets with an unchanged CPU time is contention for a core of this
     shared host (core 16 ran at cpu/wall = 0.49 here), never a speedup.
     The same sweep is run for ngspice (rawfile data compared after the header, which carries a date).
A missing binary or ngspice makes the corresponding part `unverified` (printed), never a pass.

Resources (AGENTS.md Rule 9): the biggest deck is a 60x60 mesh (3,601 unknowns, ~110 MiB RustSpice); configurations run one at a time.
Usage: check_thread_determinism.py [--rustspice BIN] [--ngspice ngspice] [--cpus 22 --cpus 22,23] [--workdir DIR] [--big]
"""
import argparse, hashlib, os, re, resource, shutil, subprocess, sys, tempfile, time

# <repo>/modules/RustSpice/benchmarks/checks/scaling_and_memory_families/<script> -> three levels up = <repo>/modules/RustSpice
ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
DEFAULT_RUSTSPICE = os.environ.get('RUSTSPICE', os.path.join(ROOT, 'target', 'release', 'rustspice'))
DEFAULT_NGSPICE = os.environ.get('NGSPICE', 'ngspice')


def mesh2d(m):
    L = [f"2-D resistor mesh {m}x{m} with leak, corner driven", "V1 n0_0 0 5"]
    for i in range(m):
        for j in range(m):
            if i + 1 < m: L.append(f"Rx{i}_{j} n{i}_{j} n{i+1}_{j} 1k")
            if j + 1 < m: L.append(f"Ry{i}_{j} n{i}_{j} n{i}_{j+1} 1k")
            L.append(f"Rg{i}_{j} n{i}_{j} 0 20k")
    return '\n'.join(L + [".op", ".end"]) + '\n'


def mesh3d(n):
    L = [f"3-D resistor mesh {n}x{n}x{n} with leak, corner driven", "V1 n0_0_0 0 5"]
    for i in range(n):
        for j in range(n):
            for k in range(n):
                if i + 1 < n: L.append(f"Rx{i}_{j}_{k} n{i}_{j}_{k} n{i+1}_{j}_{k} 1k")
                if j + 1 < n: L.append(f"Ry{i}_{j}_{k} n{i}_{j}_{k} n{i}_{j+1}_{k} 1k")
                if k + 1 < n: L.append(f"Rz{i}_{j}_{k} n{i}_{j}_{k} n{i}_{j}_{k+1} 1k")
                L.append(f"Rg{i}_{j}_{k} n{i}_{j}_{k} 0 50k")
    return '\n'.join(L + [".op", ".end"]) + '\n'


def rc_mesh_tran(m, steps=2000):
    L = [f"RC mesh {m}x{m} transient", "V1 src 0 PULSE(0 1 0.1n 0.1n 5n 20n)", "Rs src n0_0 100"]
    for i in range(m):
        for j in range(m):
            if j + 1 < m: L.append(f"Rh{i}_{j} n{i}_{j} n{i}_{j+1} 1k")
            if i + 1 < m: L.append(f"Rv{i}_{j} n{i}_{j} n{i+1}_{j} 1k")
            L.append(f"C{i}_{j} n{i}_{j} 0 1p")
    return '\n'.join(L + [f".tran 10p {steps * 10e-12 + 5e-12:.12g}", ".end"]) + '\n'


def diode_ladder_tran(steps=20000):
    L = ["diode-clamp RC ladder transient", ".model dm D(IS=1e-14 N=1)", "V1 in 0 SIN(0 3 20k)"]
    prev = 'in'
    for k in range(1, 6):
        L += [f"R{k} {prev} n{k} 1k", f"C{k} n{k} 0 1n", f"D{k} n{k} 0 dm"]
        prev = f"n{k}"
    return '\n'.join(L + [f".tran 100n {(steps + 0.5) * 100e-9:.12g}", ".end"]) + '\n'



def idle_cores(k, window=0.6):
    """The k idlest CPUs among the ones this process may use (sampled from /proc/stat over `window` s); [] if unavailable."""
    try:
        allowed = sorted(os.sched_getaffinity(0))
        def rd():
            d = {}
            for l in open('/proc/stat'):
                if l.startswith('cpu') and l[3:4].isdigit():
                    f = l.split(); d[int(f[0][3:])] = (sum(map(int, f[1:])), int(f[4]) + int(f[5]))
            return d
        a = rd(); time.sleep(window); b = rd()
        busy = {c: 1.0 - (b[c][1] - a[c][1]) / max(1, b[c][0] - a[c][0]) for c in allowed if c in a and c in b}
        return sorted(busy, key=lambda c: (busy[c], c))[:k]
    except Exception:
        return []


def sample_threads(p):
    best = 1
    while p.poll() is None:
        try:
            with open(f'/proc/{p.pid}/status') as f:
                m = re.search(r'Threads:\s+(\d+)', f.read())
            if m: best = max(best, int(m.group(1)))
        except OSError:
            pass
        time.sleep(0.003)
    return best


def run_child(cmd, cpus, env_extra, timeout=120):
    env = dict(os.environ)
    for k in ('RAYON_NUM_THREADS', 'OMP_NUM_THREADS'):
        env.pop(k, None)
    env.update(env_extra)
    full = ['taskset', '-c', cpus] + cmd if cpus else cmd
    ru0 = resource.getrusage(resource.RUSAGE_CHILDREN)
    t0 = time.monotonic()
    p = subprocess.Popen(full, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env)
    thr = sample_threads(p)
    try:
        p.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        p.kill(); p.wait()
        return None, thr, time.monotonic() - t0, 0.0, 'timeout'
    ru1 = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu = (ru1.ru_utime - ru0.ru_utime) + (ru1.ru_stime - ru0.ru_stime)
    return p.returncode, thr, time.monotonic() - t0, cpu, None


def body_hash(path, skip_header):
    b = open(path, 'rb').read()
    if skip_header:  # ngspice header carries a date; hash only what follows `Binary:\n` of every plot
        parts = [x for x in re.split(rb'Binary:\n', b)[1:]]
        # plots are concatenated: each part holds numeric data followed by the next header; keep only the numeric prefix
        h = hashlib.sha256()
        for part in parts:
            cut = part.find(b'Title:')
            h.update(part if cut < 0 else part[:cut])
        return h.hexdigest()
    return hashlib.sha256(b).hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--rustspice', default=DEFAULT_RUSTSPICE, help='default: $RUSTSPICE, else target/release/rustspice of this repository')
    ap.add_argument('--ngspice', default=DEFAULT_NGSPICE, help='default: $NGSPICE, else ngspice on PATH')
    ap.add_argument('--cpus', action='append', default=None, help='affinity sets for taskset, e.g. 22 and 22,23 (repeatable); default: the idlest core and the idlest two cores')
    ap.add_argument('--workdir', default=None)
    ap.add_argument('--big', action='store_true', help='use the 100x100 mesh (10,001 unknowns, ~650 MiB) instead of 60x60')
    a = ap.parse_args()
    if a.cpus:
        cpus_sets = a.cpus
    else:
        ic = idle_cores(2)
        cpus_sets = [str(ic[0]), f'{ic[0]},{ic[1]}'] if len(ic) == 2 else [None, None]
        print('affinity sets (auto-picked idle cores):', cpus_sets)
    envs = [('default', {}), ('RAYON1', {'RAYON_NUM_THREADS': '1'}), ('RAYON2', {'RAYON_NUM_THREADS': '2'}),
            ('RAYON4', {'RAYON_NUM_THREADS': '4'}), ('OMP1', {'OMP_NUM_THREADS': '1'}), ('OMP4', {'OMP_NUM_THREADS': '4'})]
    work = a.workdir or tempfile.mkdtemp(prefix='scaldet-', dir='/dev/shm' if os.path.isdir('/dev/shm') else None)
    os.makedirs(work, exist_ok=True)
    decks = {
        'mesh2d': mesh2d(100 if a.big else 60),
        'mesh3d': mesh3d(8),
        'rc_mesh_tran': rc_mesh_tran(10),
        'diode_ladder_tran': diode_ladder_tran(),
    }
    have_rs = shutil.which(a.rustspice) or os.path.exists(a.rustspice)
    have_ng = shutil.which(a.ngspice)
    violations = []
    unverified = []
    if not have_rs:
        print('UNVERIFIED: rustspice binary not found:', a.rustspice); sys.exit(2)
    for name, text in decks.items():
        deck = os.path.join(work, name + '.sp')
        open(deck, 'w').write(text)
        for tool in ('rustspice', 'ngspice'):
            if tool == 'ngspice' and not have_ng:
                unverified.append(f'{name}/ngspice: ngspice not found'); continue
            hashes = {}
            rows = []
            configs = [(c, en, e) for c in cpus_sets for en, e in envs] + [(cpus_sets[0], 'default-repeat', {})]
            for cpus, en, e in configs:
                raw = os.path.join(work, f'{name}.{tool}.raw')
                if os.path.exists(raw): os.remove(raw)
                if tool == 'rustspice':
                    cmd = [a.rustspice, 'run', deck, '--raw', raw]
                else:
                    cmd = [a.ngspice, '-b', '-r', raw, deck]
                rc, thr, wall, cpu, err = run_child(cmd, cpus, e)
                if rc != 0 or not os.path.exists(raw):
                    violations.append(f'{name}/{tool} cpus={cpus} env={en}: run failed rc={rc} {err or ""}')
                    continue
                h = body_hash(raw, skip_header=(tool == 'ngspice'))
                hashes.setdefault(h, []).append((cpus, en))
                rows.append((cpus, en, thr, wall, cpu, h[:10]))
            print(f'--- {name} / {tool}')
            for cpus, en, thr, wall, cpu, h in rows:
                print(f'    cpus={str(cpus):<6} env={en:<15} max_os_threads={thr:<3} wall={wall:7.3f}s cpu={cpu:7.3f}s cpu/wall={cpu / wall if wall else 0:4.2f} sha256[:10]={h}')
            if len(hashes) > 1:
                violations.append(f'{name}/{tool}: {len(hashes)} different rawfile contents across configurations: ' +
                                  '; '.join(f'{k[:10]} <- {v}' for k, v in hashes.items()))
            elif hashes:
                print(f'    DETERMINISTIC: one rawfile content across {len(rows)} configurations')
            by_cpu = {c: (w, u) for c, en, thr, w, u, h in rows if en == 'default'}
            if len(by_cpu) > 1:
                ks = list(by_cpu)
                print('    default env, per affinity set: ' + '; '.join(f'{k}: wall {by_cpu[k][0]:.3f}s cpu {by_cpu[k][1]:.3f}s' for k in ks) +
                      f'  => wall ratio {by_cpu[ks[0]][0] / by_cpu[ks[-1]][0]:.2f}x but CPU-time ratio {by_cpu[ks[0]][1] / max(1e-9, by_cpu[ks[-1]][1]):.2f}x')
            peak_thr = max(r[2] for r in rows) if rows else 0
            peak_par = max((r[4] / r[3]) for r in rows if r[3] > 0.05) if any(r[3] > 0.05 for r in rows) else 0.0
            if peak_par <= 1.1:
                print(f'    NO PARALLEL CPU USE: max cpu/wall {peak_par:.2f} with up to {peak_thr} OS threads alive; '
                      'a wall-time change between affinity sets is host contention on a core, not parallelism')
            else:
                print(f'    PARALLEL CPU USE: max cpu/wall {peak_par:.2f} with up to {peak_thr} OS threads')
    if not a.workdir:
        shutil.rmtree(work, ignore_errors=True)
    for u in unverified: print('UNVERIFIED:', u)
    if violations:
        print('\nVIOLATIONS:')
        for v in violations: print('  ', v)
        sys.exit(1)
    print('\nOK: no violations' + (' (some parts unverified)' if unverified else ''))


if __name__ == '__main__':
    main()
