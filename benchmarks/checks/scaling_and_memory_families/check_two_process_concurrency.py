#!/usr/bin/env python3
"""Two concurrent processes vs one (SUITE_SPEC section 6: shared-resource effects a single-process timing hides).

For each deck (2-D mesh DC, 3-D mesh DC, long RC-ladder transient; generated here) and each tool (rustspice, ngspice):
  solo       : the deck alone on one core, REPS repetitions (min and median wall, VmHWM)
  concurrent : two copies at the same time, each pinned to its OWN core (so there is no CPU sharing between them),
               REPS repetitions; wall per copy, VmHWM per copy
Violations (exit 1):
  * a concurrent copy exits non-zero, or its rawfile data differs from the solo run (shared global state, temp-file
    collisions, a data race; ngspice header dates are excluded),
  * median concurrent wall > --max-slowdown (default 2.0) times the solo minimum although the two copies never share a
    core (contention in the allocator, the memory system or a shared thread pool),
  * concurrent VmHWM of a copy > 1.25x the solo VmHWM (memory that scales with the number of processes).
The host is shared and noisy: the slowdown is reported with the relative spread of the solo runs and flagged `noisy` when
that spread exceeds 0.25 (a noisy ratio can only add a note, never a violation).

Resources (AGENTS.md Rule 9): two processes at a time, <= ~350 MiB each (3-D mesh n=12); everything is sequential otherwise.
Usage: check_two_process_concurrency.py [--rustspice BIN] [--ngspice ngspice] [--cpu-a 22 --cpu-b 23] [--reps 5]
"""
import argparse, hashlib, os, re, shutil, statistics, subprocess, sys, tempfile, time

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


def rc_ladder_tran(steps):
    L = ["RC ladder 10 stages, long transient", "V1 in 0 SIN(0 1 20k)"]
    prev = 'in'
    for k in range(1, 11):
        L += [f"R{k} {prev} n{k} 1k", f"C{k} n{k} 0 1n"]
        prev = f"n{k}"
    return '\n'.join(L + [f".tran 100n {(steps + 0.5) * 100e-9:.12g}", ".end"]) + '\n'


def data_hash(path, tool):
    b = open(path, 'rb').read()
    if tool == 'ngspice':
        h = hashlib.sha256()
        for part in re.split(rb'Binary:\n', b)[1:]:
            cut = part.find(b'Title:')
            h.update(part if cut < 0 else part[:cut])
        return h.hexdigest()
    return hashlib.sha256(b).hexdigest()



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


def spawn(cmd, cpu):
    t0 = time.monotonic()
    p = subprocess.Popen(['taskset', '-c', str(cpu)] + cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return p, t0


def reap(procs, timeout=180):
    """procs: list of (Popen, t0). Returns list of (rc, wall, vmhwm_mib) sampling VmHWM until all exit."""
    res = [None] * len(procs)
    hwm = [0] * len(procs)
    wall = [None] * len(procs)
    t_end = time.monotonic() + timeout
    while any(r is None for r in res):
        for i, (p, t0) in enumerate(procs):
            if res[i] is not None: continue
            try:
                with open(f'/proc/{p.pid}/status') as f:
                    m = re.search(r'VmHWM:\s+(\d+)', f.read())
                if m: hwm[i] = max(hwm[i], int(m.group(1)))
            except OSError:
                pass
            rc = p.poll()
            if rc is not None:
                res[i] = rc; wall[i] = time.monotonic() - t0
            elif time.monotonic() > t_end:
                p.kill(); p.wait(); res[i] = -9; wall[i] = time.monotonic() - t0
        time.sleep(0.004)
    return [(res[i], wall[i], hwm[i] / 1024.0) for i in range(len(procs))]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--rustspice', default=DEFAULT_RUSTSPICE, help='default: $RUSTSPICE, else target/release/rustspice of this repository')
    ap.add_argument('--ngspice', default=DEFAULT_NGSPICE, help='default: $NGSPICE, else ngspice on PATH')
    ap.add_argument('--cpu-a', default=None); ap.add_argument('--cpu-b', default=None)
    ap.add_argument('--reps', type=int, default=5)
    ap.add_argument('--max-slowdown', type=float, default=2.0)
    ap.add_argument('--workdir', default=None)
    a = ap.parse_args()
    if a.cpu_a is None or a.cpu_b is None:
        ic = idle_cores(2)
        if len(ic) < 2: print('UNVERIFIED: cannot pick two idle cores; pass --cpu-a/--cpu-b'); sys.exit(2)
        a.cpu_a, a.cpu_b = str(ic[0]), str(ic[1])
        print(f'cores (auto-picked idle): {a.cpu_a}, {a.cpu_b}')
    work = a.workdir or tempfile.mkdtemp(prefix='scalconc-', dir='/dev/shm' if os.path.isdir('/dev/shm') else None)
    os.makedirs(work, exist_ok=True)
    decks = {'mesh2d_60x60': mesh2d(60), 'mesh3d_n12': mesh3d(12), 'rc_ladder10_tran_3e5': rc_ladder_tran(300000)}
    if not (shutil.which(a.rustspice) or os.path.exists(a.rustspice)):
        print('UNVERIFIED: rustspice binary not found'); sys.exit(2)
    tools = ['rustspice'] + (['ngspice'] if shutil.which(a.ngspice) else [])
    if 'ngspice' not in tools: print('UNVERIFIED: ngspice not found, ngspice part skipped')
    violations, notes = [], []
    for name, text in decks.items():
        deck = os.path.join(work, name + '.sp'); open(deck, 'w').write(text)
        for tool in tools:
            def cmd(raw):
                return ([a.rustspice, 'run', deck, '--raw', raw] if tool == 'rustspice' else [a.ngspice, '-b', '-r', raw, deck])
            solo_w, solo_m, solo_h = [], [], None
            for r in range(a.reps):
                raw = os.path.join(work, f'{name}.{tool}.solo.raw')
                rc, w, m = reap([spawn(cmd(raw), a.cpu_a)])[0]
                if rc != 0: violations.append(f'{name}/{tool}: solo run failed rc={rc}'); break
                solo_w.append(w); solo_m.append(m)
                h = data_hash(raw, tool)
                if solo_h is None: solo_h = h
                elif h != solo_h: violations.append(f'{name}/{tool}: solo runs differ from each other (non-determinism)')
            if not solo_w: continue
            cw, cm, bad = [], [], 0
            for r in range(a.reps):
                raws = [os.path.join(work, f'{name}.{tool}.c{k}.raw') for k in (0, 1)]
                res = reap([spawn(cmd(raws[0]), a.cpu_a), spawn(cmd(raws[1]), a.cpu_b)])
                for k, (rc, w, m) in enumerate(res):
                    if rc != 0:
                        violations.append(f'{name}/{tool}: concurrent copy {k} failed rc={rc}'); bad += 1; continue
                    cw.append(w); cm.append(m)
                    if data_hash(raws[k], tool) != solo_h:
                        violations.append(f'{name}/{tool}: concurrent copy {k} rawfile differs from the solo rawfile'); bad += 1
            if not cw: continue
            smin, smed = min(solo_w), statistics.median(solo_w)
            spread = (statistics.pstdev(solo_w) / smed) if smed > 0 else 0.0
            ratio = statistics.median(cw) / smin
            noisy = spread > 0.25
            mem_ratio = max(cm) / max(1e-9, max(solo_m))
            print(f'{name:<22} {tool:<9} solo min {smin:7.3f}s med {smed:7.3f}s spread {spread:4.2f} | 2 concurrent: med {statistics.median(cw):7.3f}s '
                  f'slowdown {ratio:4.2f}x{" (noisy)" if noisy else ""} | VmHWM solo {max(solo_m):7.1f} MiB conc {max(cm):7.1f} MiB ({mem_ratio:4.2f}x) | data identical: {bad == 0}')
            if ratio > a.max_slowdown:
                msg = f'{name}/{tool}: median concurrent wall {ratio:.2f}x the solo minimum (limit {a.max_slowdown}x) with each copy on its own core'
                (notes if noisy else violations).append(msg + (' [noisy: note only]' if noisy else ''))
            if mem_ratio > 1.25:
                violations.append(f'{name}/{tool}: concurrent VmHWM {max(cm):.1f} MiB is {mem_ratio:.2f}x the solo {max(solo_m):.1f} MiB')
    if not a.workdir: shutil.rmtree(work, ignore_errors=True)
    for n in notes: print('NOTE:', n)
    if violations:
        print('\nVIOLATIONS:')
        for v in violations: print('  ', v)
        sys.exit(1)
    print('\nOK: no violations')


if __name__ == '__main__':
    main()
