#!/usr/bin/env python3
"""Reproduces every number quoted in the notes of the transient relation decks (gen_tran_decks.py).

tran_superposition_rcladder30: |r - 1| over all rows for RustSpice and for ngspice (reference options, tmax = tstep/20),
    swing of the waveform probes and RustSpice-vs-ngspice error of the waveform probes (resampled on the tstep grid).
tran_rest_equals_op_cmos5: |v_tran - v_op| of ngspice's flat reference and of RustSpice per probe and the .op difference
    RustSpice vs ngspice-tight for all stages (the gain growth per stage).
Usage: calibrate_tran_decks.py [deckdir] [workdir]     deckdir holds the generated acc_rel_tran_*.sp (default: benchmarks/cases/accuracy)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np  # noqa: E402

from rfr_common import CASES, default_work, col, last_plot, run_ng, run_rs  # noqa: E402

DECKS = sys.argv[1] if len(sys.argv) > 1 else os.path.join(CASES, "accuracy")
W = sys.argv[2] if len(sys.argv) > 2 else default_work("cal_tran")
os.makedirs(W, exist_ok=True)
REF = "reltol=1e-6 abstol=1e-15 vntol=1e-9"  # + tmax=tstep/20 injected below


def with_tmax(path, tmax):
    t = open(path).read()
    out = []
    for ln in t.splitlines():
        if ln.lower().startswith(".tran"):
            ln += f" 0 {tmax:g}"  # .tran tstep tstop tstart tmax
        out.append(ln)
    p = os.path.join(W, "ref_" + os.path.basename(path))
    open(p, "w").write("\n".join(out) + "\n")
    return p


def interp(t, y, tg):
    return np.interp(tg, t, y)


name = "acc_rel_tran_superposition_rcladder30"
f = os.path.join(DECKS, name + ".sp")
plots, st, _ = run_rs(f, W, tag="sup_rs")
rs = last_plot(plots, "Transient")
# tmax is the 4th .tran field: `.tran tstep tstop tstart tmax`
ngp = last_plot(run_ng(with_tmax(f, 10e-12 / 20), W, tag="sup_ng", options=REF)[0], "Transient")
print(f"== {name}: tier {st['solver_kind']}, {len(rs['vars'])} variables, rustspice rows {rs['data'].shape[0]}, ngspice rows {ngp['data'].shape[0]}")
t_rs, t_ng = col(rs, "time"), col(ngp, "time")
grid = np.arange(0, 5e-9 + 1e-15, 10e-12)
for r in ("v(r_10)", "v(r_30)"):
    print(f"   {r}: max|r-1| rustspice {np.abs(col(rs, r) - 1).max():.2e}   ngspice {np.abs(col(ngp, r) - 1).max():.2e}")
for v in ("v(f10)", "v(f30)"):
    a, b = interp(t_rs, col(rs, v), grid), interp(t_ng, col(ngp, v), grid)
    sw = np.ptp(b)
    print(f"   {v}: swing {sw:.3f} V, max|rs - ng| on the 10 ps grid {np.abs(a - b).max():.2e} V = {np.abs(a - b).max() / sw:.2e} of swing")

name = "acc_rel_tran_rest_equals_op_cmos5"
f = os.path.join(DECKS, name + ".sp")
plots, st, _ = run_rs(f, W, tag="rest_rs")
rs = last_plot(plots, "Transient")
ngt = last_plot(run_ng(with_tmax(f, 10e-12 / 20), W, tag="rest_ng", options=REF)[0], "Transient")
fo = os.path.join(W, "rest_op.sp")
open(fo, "w").write(open(f).read().replace(".tran 10p 2n", ".op"))
ngo = last_plot(run_ng(fo, W, tag="rest_ngop")[0], "Operating")
rso = last_plot(run_rs(fo, W, tag="rest_rsop")[0], "Operating")
print(f"== {name}: tier {st['solver_kind']}, rustspice rows {rs['data'].shape[0]}, ngspice rows {ngt['data'].shape[0]}")
print("   node   v_op(ngspice-tight)   max|v_rs(t) - v_ng_op|   max|v_ng_tran - v_ng_op|   |op_rs - op_ng|")
for tag in "abc":
    for k in range(1, 6):
        v = f"v({tag}{k})"
        o = col(ngo, v)[0]
        print(f"   {v:6s} {o:16.9f}   {np.abs(col(rs, v) - o).max():18.2e}   {np.abs(col(ngt, v) - o).max():20.2e}   {abs(col(rso, v)[0] - o):14.2e}")
