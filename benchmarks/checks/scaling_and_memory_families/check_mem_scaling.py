#!/usr/bin/env python3
"""Peak-memory scaling of `rustspice run` (SUITE_SPEC section 6 / AGENTS.md Rule 9): memory per unknown and per output element.

Peak resident memory is VmHWM (RustSpice: the larger of the 3 ms-polled /proc/<pid>/status value and the run's own `--stats` peak_rss_kib; ngspice: polled; never wait4 ru_maxrss). Each measured process is run alone, pinned to
one core, under the caller's cgroup limit. The start-up RSS of the same binary on a 1-resistor deck is subtracted (`base`).

Part A, 2-D resistor meshes (m x m, natural numbering, V1 on the corner, 1k edges, 20k leak per node): unknowns = m^2 + 1.
Part B, 3-D resistor meshes (n x n x n):                                                            unknowns = n^3 + 1.
  For every size: RustSpice RSS and ngspice-KLU RSS, KB per unknown, and the least-squares log-log exponent of the RSS in the unknowns.
  Violation (exit 1): the RustSpice exponent exceeds the natural-order BAND-LU bound (a direct solver that does nothing smarter than
  banding stores n * bandwidth numbers: bandwidth sqrt(n) in 2-D -> exponent 1.5, n^(2/3) in 3-D -> exponent 5/3). Any fill-reducing
  ordering is below that bound; ngspice-KLU's measured exponent is printed next to it. Thresholds are --max-exp-2d / --max-exp-3d.
Part C, transients of an RC ladder (W stages, tstep 100 ns): RustSpice keeps every output row in memory, ngspice streams.
  For (W, steps) in the grid: raw file bytes, RustSpice RSS - base, ratio (RSS - base) / raw bytes, ngspice RSS, and the extrapolated RSS
  of a 1e7-step x 100-stage run. Violation (exit 1): ratio > --max-tran-ratio (default 2.5: more than two buffered copies of the output).
  The extrapolation is information only (there is no per-run memory budget to violate; that is the `resource-budget` gap).
A fit with fewer than 3 points above the floor is `unverified` (printed, exit 2 if nothing else failed), never a pass.

Memory estimate (Rule 9): largest runs are 2-D m=100 (~600 MiB) and 3-D n=14 (~720 MiB) RustSpice, run one after the other.
Usage: check_mem_scaling.py [--rustspice BIN] [--ngspice ngspice] [--cpu 22] [--quick]
"""
import argparse, math, os, re, shutil, subprocess, sys, tempfile, time

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


def rc_ladder_tran(width, steps):
    L = [f"RC ladder {width} stages, {steps} steps", "V1 in 0 SIN(0 1 20k)"]
    prev = 'in'
    for k in range(1, width + 1):
        L += [f"R{k} {prev} n{k} 1k", f"C{k} n{k} 0 1n"]
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


def measure(cmd, cpu, timeout=300):
    """Run cmd pinned to cpu; return (rc, wall_s, vmhwm_mib)."""
    t0 = time.monotonic()
    p = subprocess.Popen(['taskset', '-c', str(cpu)] + cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    hwm = 0
    while p.poll() is None:
        try:
            with open(f'/proc/{p.pid}/status') as f:
                m = re.search(r'VmHWM:\s+(\d+)', f.read())
            if m: hwm = max(hwm, int(m.group(1)))
        except OSError:
            pass
        if time.monotonic() - t0 > timeout:
            p.kill(); p.wait(); return -9, time.monotonic() - t0, hwm / 1024.0
        time.sleep(0.003)
    return p.returncode, time.monotonic() - t0, hwm / 1024.0


def fit_exponent(xs, ys, floor_mib=30.0):
    pts = [(math.log(x), math.log(y)) for x, y in zip(xs, ys) if y >= floor_mib]
    if len(pts) < 3: return None
    mx = sum(p[0] for p in pts) / len(pts); my = sum(p[1] for p in pts) / len(pts)
    den = sum((p[0] - mx) ** 2 for p in pts)
    return sum((p[0] - mx) * (p[1] - my) for p in pts) / den if den > 0 else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--rustspice', default=DEFAULT_RUSTSPICE, help='default: $RUSTSPICE, else target/release/rustspice of this repository')
    ap.add_argument('--ngspice', default=DEFAULT_NGSPICE, help='default: $NGSPICE, else ngspice on PATH')
    ap.add_argument('--cpu', default=None, help='core to pin to (default: the idlest allowed core)')
    ap.add_argument('--max-exp-2d', type=float, default=1.5)
    ap.add_argument('--max-exp-3d', type=float, default=5.0 / 3.0)
    ap.add_argument('--max-tran-ratio', type=float, default=2.5)
    ap.add_argument('--fit-floor-mib', type=float, default=30.0)
    ap.add_argument('--quick', action='store_true', help='smaller grids (2-D 30/50/70, 3-D 8/10/12, tran 10x1e5 and 50x2e5)')
    ap.add_argument('--workdir', default=None)
    a = ap.parse_args()
    if not (shutil.which(a.rustspice) or os.path.exists(a.rustspice)):
        print('UNVERIFIED: rustspice binary not found'); sys.exit(2)
    have_ng = bool(shutil.which(a.ngspice))
    if a.cpu is None:
        ic = idle_cores(1)
        a.cpu = str(ic[0]) if ic else '0'
        print(f'core (auto-picked idle): {a.cpu}')
    work = a.workdir or tempfile.mkdtemp(prefix='scalmem-', dir='/dev/shm' if os.path.isdir('/dev/shm') else None)
    os.makedirs(work, exist_ok=True)

    def run_rs(text, tag, raw=False):
        deck = os.path.join(work, tag + '.sp'); open(deck, 'w').write(text)
        out = os.path.join(work, tag + '.rs.raw'); st = os.path.join(work, tag + '.rs.json')
        rc, w, m = measure([a.rustspice, 'run', deck, '--raw', out, '--stats', st], a.cpu)
        try:   # the run's own end-of-run VmHWM (a 3 ms poll misses the peak of a run that lasts a few ms)
            import json
            own = json.load(open(st)).get('peak_rss_kib')
            if own: m = max(m, own / 1024.0)
        except (OSError, ValueError):
            pass
        size = os.path.getsize(out) if os.path.exists(out) else None
        for f in (out, st):
            if os.path.exists(f): os.remove(f)
        return rc, w, m, size

    def run_ng(text, tag, klu=True):
        deck = os.path.join(work, tag + '.ng.cir')
        i = text.rfind('.end')
        open(deck, 'w').write(text[:i] + ('.options klu\n' if klu else '') + text[i:])
        out = os.path.join(work, tag + '.ng.raw')
        rc, w, m = measure([a.ngspice, '-b', '-r', out, deck], a.cpu)
        if os.path.exists(out): os.remove(out)
        return rc, w, m

    violations, unverified = [], []
    rc, w, base_rs, _ = run_rs(rc_ladder_tran(10, 1000), 'base')
    base_ng = run_ng(rc_ladder_tran(10, 1000), 'baseng', klu=False)[2] if have_ng else 0.0
    print(f'start-up peak on a 10-stage 1,000-step ladder transient: rustspice {base_rs:.1f} MiB, ngspice {base_ng:.1f} MiB (subtracted in part C only)\n')

    for label, gen, sizes, unk, bound in (
            ('2-D mesh', mesh2d, [30, 50, 70] if a.quick else [30, 50, 70, 100], lambda m: m * m + 1, a.max_exp_2d),
            ('3-D mesh', mesh3d, [8, 10, 12] if a.quick else [8, 10, 12, 14], lambda n: n ** 3 + 1, a.max_exp_3d)):
        print(f'== {label}: size, unknowns, rustspice peak MiB (KB/unknown), ngspice-KLU peak MiB (KB/unknown), rustspice wall')
        xs, rs_y, ng_y = [], [], []
        for s in sizes:
            text = gen(s)
            rc, w, m, _ = run_rs(text, f'{label[:3]}{s}')
            if rc != 0:
                violations.append(f'{label} {s}: rustspice run failed rc={rc}'); continue
            ng_m = None
            if have_ng:
                rcn, wn, ng_m = run_ng(text, f'{label[:3]}{s}n')
                if rcn != 0: ng_m = None
            u = unk(s); xs.append(u); rs_y.append(m); ng_y.append(ng_m)
            print(f'   {s:>4} {u:>7} {m:9.1f} ({m * 1024 / u:7.1f})   ' +
                  (f'{ng_m:7.1f} ({ng_m * 1024 / u:6.1f})' if ng_m is not None else '    n/a') + f'   {w:7.2f}s')
        e_rs = fit_exponent(xs, rs_y, a.fit_floor_mib)
        e_ng = fit_exponent([x for x, y in zip(xs, ng_y) if y is not None], [y for y in ng_y if y is not None], a.fit_floor_mib)
        if e_rs is None:
            unverified.append(f'{label}: fewer than 3 sizes with a peak above {a.fit_floor_mib:.0f} MiB; no exponent'); continue
        tail = (math.log(rs_y[-1] / rs_y[-2]) / math.log(xs[-1] / xs[-2])) if len(xs) >= 2 and rs_y[-2] > 0 else None
        print(f'   log-log exponent of the peak RSS in the unknowns: rustspice {e_rs:.2f}' + (f' (largest two sizes {tail:.2f})' if tail is not None else '') +
              (f', ngspice-KLU {e_ng:.2f}' if e_ng is not None else '') + f'   (natural-order band-LU bound {bound:.2f})')
        if e_rs > bound:
            violations.append(f'{label}: RustSpice memory exponent {e_rs:.2f} exceeds the band-LU bound {bound:.2f} '
                              f'(largest size: {rs_y[-1] * 1024 / xs[-1]:.0f} KB per unknown)')
        print()

    print('== transient output memory: width x steps, raw MiB, rustspice VmHWM-base MiB, ratio, ngspice VmHWM MiB')
    grid = [(10, 100000), (50, 200000)] if a.quick else [(10, 100000), (10, 1000000), (50, 200000), (100, 300000)]
    per_elem = []
    for wdt, steps in grid:
        text = rc_ladder_tran(wdt, steps)
        rc, w, m, size = run_rs(text, f'tr{wdt}_{steps}')
        if rc != 0 or not size:
            violations.append(f'tran {wdt}x{steps}: rustspice run failed rc={rc}'); continue
        ratio = (m - base_rs) / (size / 2 ** 20)
        ng_m = None
        if have_ng:
            rcn, wn, ng_m = run_ng(text, f'tr{wdt}_{steps}n', klu=False)
            if rcn != 0: ng_m = None
        elems = (wdt + 3) * (steps + 2)          # time, v(in), wdt node voltages, i(v1)
        per_elem.append((m - base_rs) * 2 ** 20 / elems)
        print(f'   {wdt:>3} x {steps:>8}: raw {size / 2 ** 20:8.1f} MiB, rustspice {m:8.1f} MiB (base-subtracted ratio {ratio:4.2f}), ' +
              (f'ngspice {ng_m:6.1f} MiB' if ng_m is not None else 'ngspice n/a') + f', wall {w:6.2f}s')
        if ratio > a.max_tran_ratio:
            violations.append(f'tran {wdt}x{steps}: RSS - base is {ratio:.2f}x the output size (limit {a.max_tran_ratio}x)')
    if per_elem:
        b = sum(per_elem) / len(per_elem)
        print(f'   bytes of RustSpice RSS per output element (mean {b:.1f} B) -> a 1e7-step x 100-stage run (1.03e9 elements) would need about '
              f'{b * 1.03e9 / 2 ** 30:.1f} GiB; ngspice stays flat (information only, no budget is violated)')
    if not a.workdir: shutil.rmtree(work, ignore_errors=True)
    for u in unverified: print('UNVERIFIED:', u)
    if violations:
        print('\nVIOLATIONS:')
        for v in violations: print('  ', v)
        sys.exit(1)
    if unverified: sys.exit(2)
    print('\nOK: no violations')


if __name__ == '__main__':
    main()
