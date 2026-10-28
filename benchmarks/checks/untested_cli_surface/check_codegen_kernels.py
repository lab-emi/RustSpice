#!/usr/bin/env python3
"""`rustspice codegen --model {diode,mosfet-l1,mosfet-ekv} --target {rust,cuda-tile,cuda-ptx,msl,wgsl}` evaluated, not just printed.

  rust      the emitted `eval_<model>_scalar` is compiled with rustc and run on a grid. Values are compared with closed forms in
            50-digit mpmath (diode: Shockley + gmin; level-1: Shichman-Hodges incl. lambda and gmin; EKV: the formula documented
            in the IR builder), every analytic derivative output (g_d, c_d, d_id_d_is/n, g_m, g_ds, d_id_d_w/l/vto) with mpmath's
            numerical derivative of the closed form AND with a central finite difference of the compiled kernel itself.
  netlist   the diode and level-1 kernels are compared with -i(v1) of tightly converged black-box ngspice on two generated
            reference decks (diode directly across a swept source; NMOS Id-Vgs through cutoff/saturation/triode), and the EKV kernel with the live RustSpice
            solution of a LEVEL=2 common-source stage (kernel and simulator are one model).
  cuda-tile real GPU: the emitted CUDA C++ is compiled with NVRTC, loaded with the driver API and run on the RTX device in fp64;
            all six outputs must equal the compiled-Rust kernel to 1e-12 relative (skipped => UNVERIFIED without cuda-python/GPU).
  cuda-ptx  the emitted PTX must contain stores to its output pointers.  KNOWN GAP codegen-ptx-empty-kernel: it is an empty kernel
            (body = `ret;`).  Static proof (no st.* instruction) and, with a GPU, dynamic proof (sentinel outputs stay untouched).
  msl/wgsl  cannot be executed here; every target is parsed into its SSA statement list and must equal the rust list op by op.

Exit code 0 = no unexpected violation; 1 = unexpected violation; 2 = prerequisites (rustc) missing.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _clicommon import *  # noqa: E402,F403

PREFIX = "cap_cli_"
# (input names, param names, output names) in IR declaration order (RustSpice rustspice-ir/src/models.rs; verified behaviourally below)
MODELS = {
    "diode": (["v_d"], ["is", "n", "vt", "gmin", "cjo", "tt"], ["i_d", "q_d", "g_d", "c_d", "d_id_d_is", "d_id_d_n"], "eval_diode_scalar", "eval_diode_tile"),
    "mosfet-l1": (["v_gs", "v_ds"], ["kp", "w", "l", "vto", "lambda", "gmin"], ["i_d", "g_m", "g_ds", "d_id_d_w", "d_id_d_l", "d_id_d_vto"], "eval_mosfet_level1_scalar", "eval_mosfet_level1_tile"),
    "mosfet-ekv": (["v_gs", "v_ds"], ["kp", "w", "l", "vto", "n", "vt", "lambda"], ["i_d", "g_m", "g_ds", "d_id_d_w", "d_id_d_l", "d_id_d_vto"], "eval_mosfet_ekv_scalar", "eval_mosfet_ekv_tile"),
}
DEF = {  # IR default parameter values
    "diode": dict(is_=1e-14, n=1.0, vt=0.025851997, gmin=1e-12, cjo=0.0, tt=0.0),
    "mosfet-l1": dict(kp=120e-6, w=10e-6, l=1e-6, vto=0.7, lam=0.02, gmin=1e-12),
    "mosfet-ekv": dict(kp=120e-6, w=10e-6, l=1e-6, vto=0.7, n=1.3, vt=0.025851997, lam=0.02),
}
VALUE_REL_MODEL = {"diode": 1e-12, "mosfet-l1": 1e-12, "mosfet-ekv": 1e-11}  # EKV: ln^2 difference cancels ~2 digits at small vds
DERIV_REL_CF, DERIV_REL_FD = 1e-10, 1e-6
INJECT = {"rel": 0.0}  # --inject-error: scale output #3 of every evaluated kernel (self-test of the gates)
TILE_REL = 1e-12


class Report:
    def __init__(self):
        self.rows, self.bad = [], 0

    def add(self, name, status, detail):
        self.rows.append({"check": name, "status": status, "detail": detail})
        if status.startswith("FAIL"):
            self.bad += 1
        print(f"[{status:<19}] {name}: {detail}")


# ------------------------------------------------------------------------------------------- closed forms (mpmath)
def cf_diode(x, p):
    v, (is_, n, vt, gmin, cjo, tt) = x[0], p
    e = mp.exp(max(-80, min(80, v / (n * vt))))
    ish = is_ * (e - 1)
    return [ish + gmin * v, tt * ish + cjo * v]


def cf_l1(x, p):
    vgs, vds = x
    kp, w, l, vto, lam, gmin = p
    beta, vov, clm = kp * w / l, vgs - vto, 1 + lam * vds
    if vov <= 0:
        i = mp.mpf(0)
    elif vds > vov:
        i = beta / 2 * vov**2 * clm
    else:
        i = beta * (vov * vds - vds**2 / 2) * clm
    return [i + gmin * vds]


def cf_ekv(x, p):
    vgs, vds = x
    kp, w, l, vto, n, vt, lam = p
    beta = kp * w / l
    vp = (vgs - vto) / n
    i_f = mp.log(1 + mp.exp(max(-80, min(80, vp / (2 * vt))))) ** 2
    i_r = mp.log(1 + mp.exp(max(-80, min(80, (vp - vds) / (2 * vt))))) ** 2
    return [2 * n * beta * vt**2 * (i_f - i_r) * (1 + lam * vds)]


CF = {"diode": cf_diode, "mosfet-l1": cf_l1, "mosfet-ekv": cf_ekv}


def cf_all(model, x, p):
    """closed-form values and derivative outputs, in the kernel's output order (derivatives by mp.diff on the closed form)."""
    f = CF[model]
    xs, ps = [mp.mpf(repr(float(t))) for t in x], [mp.mpf(repr(float(t))) for t in p]
    base = f(xs, ps)
    if model == "diode":
        is_, n = ps[0], ps[1]
        i_d, q_d = base
        d = lambda k: mp.diff(lambda t: f([t], ps)[0], xs[0])
        g_d = d(0)
        c_d = mp.diff(lambda t: f([t], ps)[1], xs[0])
        dis = mp.diff(lambda t: f(xs, [t] + ps[1:])[0], ps[0])
        dn = mp.diff(lambda t: f(xs, [ps[0], t] + ps[2:])[0], ps[1])
        return [i_d, q_d, g_d, c_d, dis, dn]
    i_d = base[0]
    gm = mp.diff(lambda t: f([t, xs[1]], ps)[0], xs[0])
    gds = mp.diff(lambda t: f([xs[0], t], ps)[0], xs[1])
    iw = {"mosfet-l1": 1, "mosfet-ekv": 1}[model]
    il = 2
    ivto = 3
    dw = mp.diff(lambda t: f(xs, ps[:iw] + [t] + ps[iw + 1:])[0], ps[iw])
    dl = mp.diff(lambda t: f(xs, ps[:il] + [t] + ps[il + 1:])[0], ps[il])
    dv = mp.diff(lambda t: f(xs, ps[:ivto] + [t] + ps[ivto + 1:])[0], ps[ivto])
    return [i_d, gm, gds, dw, dl, dv]


# ---------------------------------------------------------------------------------------------- compile & run
def compile_rust(code, model, tmp, rustc):
    ins, pars, outs, fn, _ = MODELS[model]
    prog = code + f"""
use std::io::BufRead;
fn main() {{
    let (nin, nout) = ({len(ins)}usize, {len(outs)}usize);
    for line in std::io::stdin().lock().lines() {{
        let line = line.unwrap();
        let v: Vec<f64> = line.split_whitespace().map(|t| f64::from_bits(u64::from_str_radix(t, 16).unwrap())).collect();
        let mut out = vec![0.0f64; nout];
        {fn}(&v[..nin], &v[nin..], &mut out);
        println!("{{}}", out.iter().map(|x| format!("{{:016x}}", x.to_bits())).collect::<Vec<_>>().join(" "));
    }}
}}
"""
    src, exe = tmp.f(f"k_{model}.rs"), tmp.f(f"k_{model}")
    open(src, "w").write(prog)
    rc, out, err, _ = run([rustc, "--edition", "2021", "-C", "opt-level=2", "-A", "warnings", "-o", exe, src], timeout=120)
    if rc != 0:
        raise RuntimeError(f"rustc failed: {err[-400:]}")
    return exe


def to_hex(x):
    return struct.pack(">d", float(x)).hex()


def kernel_eval(exe, rows):
    text = "\n".join(" ".join(to_hex(t) for t in r) for r in rows) + "\n"
    p = subprocess.run([exe], input=text.encode(), capture_output=True, timeout=60)
    res = []
    for ln in p.stdout.decode().strip().splitlines():
        res.append([struct.unpack(">d", bytes.fromhex(h))[0] for h in ln.split()])
    if INJECT["rel"]:
        for r in res:
            r[3] *= 1 + INJECT["rel"]
    return res


def grid(model):
    d = DEF[model]
    pr = []  # (inputs, params)
    if model == "diode":
        base = [d["is_"], d["n"], d["vt"], d["gmin"], d["cjo"], d["tt"]]
        for v in (-0.5, -0.05, 0.0, 0.2, 0.45, 0.6, 0.7, 0.8, 1.0):
            pr.append(([v], base))
        for v in (0.55, 0.65):  # charge/capacitance terms and n != 1
            pr.append(([v], [3e-15, 1.35, 0.025864917007155805, 1e-12, 2e-12, 5e-9]))
    elif model == "mosfet-l1":
        base = [d["kp"], d["w"], d["l"], d["vto"], d["lam"], d["gmin"]]
        for vgs in (0.3, 0.9, 1.2, 1.8, 2.5, 3.3):
            for vds in (0.05, 0.4, 0.8, 1.5, 3.0):
                pr.append(([vgs, vds], base))
        pr.append(([0.2, 1.0], [30e-6, 3e-6, 0.5e-6, -0.4, 0.0, 1e-12]))  # depletion device, lambda = 0
        pr.append(([1.4, 0.3], [30e-6, 3e-6, 0.5e-6, -0.4, 0.05, 1e-12]))
    else:
        base = [d["kp"], d["w"], d["l"], d["vto"], d["n"], d["vt"], d["lam"]]
        for vgs in (0.2, 0.5, 0.7, 0.9, 1.5, 3.0):
            for vds in (0.02, 0.3, 1.0, 3.0):
                pr.append(([vgs, vds], base))
    return pr


def rel_err(a, b, floor):
    return abs(a - b) / max(abs(b), floor)


def check_rust(rep, args, tmp, kernels):
    exes = {}
    for model, (ins, pars, outs, fn, _) in MODELS.items():
        code = kernels[(model, "rust")]
        if f"pub fn {fn}(" not in code:
            rep.add(f"codegen/{model}/rust/signature", "FAIL", f"{fn} not found")
            continue
        exes[model] = exe = compile_rust(code, model, tmp, args.rustc)
        pts = grid(model)
        rows = [i + p for i, p in pts]
        got = kernel_eval(exe, rows)
        n_bad = 0
        worst = {"value": 0.0, "deriv_cf": 0.0, "deriv_fd": 0.0}
        for (xin, par), g in zip(pts, got):
            ref = cf_all(model, xin, par)
            # value (output 0, plus q_d for the diode)
            for k in ((0, 1) if model == "diode" else (0,)):
                e = rel_err(g[k], float(ref[k]), 1e-30)
                worst["value"] = max(worst["value"], e)
                if e > VALUE_REL_MODEL[model]:
                    n_bad += 1
                    rep.add(f"codegen/{model}/rust/value[{outs[k]}]@{xin}", "FAIL", f"kernel={g[k]!r} closed_form={float(ref[k])!r} rel={e:.1e}")
            scale_i = max(abs(float(ref[0])), 1e-30)
            for k in range(1 if model != "diode" else 2, len(outs)):
                if outs[k] == "q_d":
                    continue
                e = abs(g[k] - float(ref[k])) / max(abs(float(ref[k])), 1e-30 * 0 + 1e-18)
                worst["deriv_cf"] = max(worst["deriv_cf"], e)
                if e > DERIV_REL_CF:
                    n_bad += 1
                    rep.add(f"codegen/{model}/rust/{outs[k]}@{xin}", "FAIL", f"kernel={g[k]!r} dClosedForm={float(ref[k])!r} rel={e:.1e}")
            # AD vs central finite difference of the compiled kernel itself (independent of the closed form)
            fd_rows, fd_idx = [], []
            nin = len(xin)
            names = ins + pars
            vec = list(xin) + list(par)
            tgt = {"diode": {"g_d": 0, "c_d": 0, "d_id_d_is": nin + 0, "d_id_d_n": nin + 1},
                   "mosfet-l1": {"g_m": 0, "g_ds": 1, "d_id_d_w": nin + 1, "d_id_d_l": nin + 2, "d_id_d_vto": nin + 3},
                   "mosfet-ekv": {"g_m": 0, "g_ds": 1, "d_id_d_w": nin + 1, "d_id_d_l": nin + 2, "d_id_d_vto": nin + 3}}[model]
            for name, j in tgt.items():
                h = 1e-6 * max(abs(vec[j]), 1e-3 if j < nin else 0)
                if h == 0:
                    h = 1e-9
                vp, vm = list(vec), list(vec)
                vp[j] += h
                vm[j] -= h
                fd_rows += [vp, vm]
                fd_idx.append((name, h))
            res = kernel_eval(exe, fd_rows)
            for n_, ((name, h), (rp, rm)) in enumerate(zip(fd_idx, zip(res[0::2], res[1::2]))):
                k_out = outs.index(name)
                src = 1 if name == "c_d" else 0
                fd = (rp[src] - rm[src]) / (2 * h)
                # rounding noise of a central difference of values of size |i|: 4 eps |i| / (2h); truncation is O(h^2) and negligible
                noise = 4 * 2.2e-16 * max(abs(rp[src]), abs(rm[src])) / (2 * h)
                ratio = abs(g[k_out] - fd) / (DERIV_REL_FD * abs(g[k_out]) + noise + 1e-300)
                worst["deriv_fd"] = max(worst["deriv_fd"], ratio)
                if ratio > 1.0:
                    n_bad += 1
                    rep.add(f"codegen/{model}/rust/{name}_vs_kernel_fd@{xin}", "FAIL", f"AD={g[k_out]!r} kernelFD={fd!r} (gate {DERIV_REL_FD:g}*|AD| + rounding noise {noise:.1e})")
        rep.add(
            f"codegen/{model}/rust",
            "PASS" if n_bad == 0 else "FAIL",
            f"{len(pts)} points x {len(outs)} outputs: worst value rel {worst['value']:.1e} (gate {VALUE_REL_MODEL[model]:g}), derivative vs closed form {worst['deriv_cf']:.1e} (gate {DERIV_REL_CF:g}), vs kernel central FD: worst use of the gate {worst['deriv_fd']:.2f} (gate {DERIV_REL_FD:g}*|AD| + FD rounding noise)",
        )
    return exes


# reference decks are generated here (not shipped as corpus decks: they would duplicate acc_dc_diode_sweep_knee_direct and
# cap_dev_mos_idvgs). The source sits directly across the device, so i(v1) is the device current itself.
DIODE_IV_DECK = "cap_cli diode IV reference\nV1 a 0 DC 0.5\nD1 a 0 DM\n.model DM D(IS=1e-14 N=1)\n.dc V1 0.30 0.78 0.02\n.end\n"
MOS_VGS_DECK = ("cap_cli nmos level-1 Id-Vgs reference at Vds = 1 V (IR default parameter vector)\nV1 d 0 DC 1.0\nV2 g 0 DC 0\nM1 d g 0 0 NM W=10u L=1u\n"
                ".model NM NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)\n.dc V2 0 3.3 0.1\n.end\n")


def check_netlists(rep, args, tmp, exes):
    t = DIODE_IV_DECK
    raw = tmp.f("ng_iv.raw")
    run([args.ngspice, "-b", "-r", raw, _opt_deck(t, tmp, "iv")], timeout=30)
    pl = read_raw(raw)[-1]
    v, i = pl["data"][:, 0], pl["data"][:, pl["names"].index("i(v1)")]
    rows = [[x, 1e-14, 1.0, VT_300K15, 1e-12, 0.0, 0.0] for x in v]
    got = kernel_eval(exes["diode"], rows)
    worst = max(abs(g[0] + ii) / max(abs(ii), 1e-30) for g, ii in zip(got, i))
    rep.add("codegen/diode/vs_ngspice_dc", "PASS" if worst < 1e-9 else "FAIL", f"{len(v)} sweep points, -i(v1) of tight ngspice vs kernel i_d: worst rel {worst:.1e} (gate 1e-9, sweep {v[0]:.2f}..{v[-1]:.2f} V, I = {abs(i).min():.1e}..{abs(i).max():.1e} A)")
    t = MOS_VGS_DECK
    raw = tmp.f("ng_mos.raw")
    run([args.ngspice, "-b", "-r", raw, _opt_deck(t, tmp, "mos")], timeout=30)
    pl = read_raw(raw)[-1]
    vg, i = pl["data"][:, 0], pl["data"][:, pl["names"].index("i(v1)")]
    rows = [[x, 1.0, 120e-6, 10e-6, 1e-6, 0.7, 0.02, 1e-12] for x in vg]
    rows = [[r[0], r[1]] + r[2:] for r in rows]
    got = kernel_eval(exes["mosfet-l1"], rows)
    err = [abs(g[0] + ii) for g, ii in zip(got, i)]
    ok = all(e <= 5e-12 + 1e-9 * abs(ii) for e, ii in zip(err, i))
    regions = {"cutoff": sum(1 for x in vg if x < 0.7), "saturation": sum(1 for x in vg if 0.7 < x < 1.7), "triode": sum(1 for x in vg if x > 1.7)}
    rep.add("codegen/mosfet-l1/vs_ngspice_dc", "PASS" if ok else "FAIL", f"{len(vg)} Vgs points {regions}, worst |kernel - (-i(v1))| = {max(err):.1e} A (gate 5e-12 + 1e-9 rel), peak current {abs(i).max():.2e} A")
    # EKV kernel against the live RustSpice solution of the identical LEVEL=2 stage
    deck = tmp.f("ekv.sp")
    open(deck, "w").write("cap_cli level2 stage\nV1 vdd 0 DC 3.3\nV2 g 0 DC 0.9\nR1 vdd d 20k\nM1 d g 0 0 NM W=10u L=1u\n.model NM NMOS(LEVEL=2 VTO=0.7 KP=120u LAMBDA=0.04)\n.op\n.end\n")
    vec = rustspice_op(open(deck).read(), tmp, binary=args.bin)
    vd = vec["v(d)"]
    i_res = (3.3 - vd) / 20e3
    k = kernel_eval(exes["mosfet-ekv"], [[0.9, vd, 120e-6, 10e-6, 1e-6, 0.7, 1.3, VT_300K15, 0.04]])[0][0] + 1e-12 * vd
    e = abs(k - i_res) / i_res
    rep.add("codegen/mosfet-ekv/vs_simulator_op", "PASS" if e < 1e-9 else "FAIL", f"kernel i_d + gmin*vds = {k:.12e} A, resistor current of the RustSpice solution = {i_res:.12e} A, rel {e:.1e} (gate 1e-9)")
    # IR default vt (300 K) differs from the simulator's 300.15 K value: a kernel used with its defaults is off by (informational)
    a = kernel_eval(exes["diode"], [[0.7, 1e-14, 1.0, 0.025851997, 1e-12, 0.0, 0.0]])[0][0]
    b = kernel_eval(exes["diode"], [[0.7, 1e-14, 1.0, VT_300K15, 1e-12, 0.0, 0.0]])[0][0]
    rep.add("info/diode_default_vt_vs_simulator_vt", "INFO", f"IR default vt=0.025851997 gives i_d(0.7 V) {a/b - 1:+.2%} versus the simulator's vt={VT_300K15:.9f}: pass vt explicitly")


def _opt_deck(text, tmp, tag):
    p = tmp.f(f"{tag}_tight.sp")
    open(p, "w").write(with_options(text, TIGHT))
    return p


# ------------------------------------------------------------------------------------------------ GPU (optional)
def gpu_tools():
    try:
        from cuda.bindings import driver as cu, nvrtc  # type: ignore
    except Exception as ex:  # noqa: BLE001
        return None, f"cuda-python unavailable ({type(ex).__name__})"

    def chk(r):
        if int(r[0]) != 0:
            raise RuntimeError(f"cuda error {r[0]}")
        return r[1:] if len(r) > 2 else (r[1] if len(r) == 2 else None)

    try:
        chk(cu.cuInit(0))
        dev = chk(cu.cuDeviceGet(0))
        ctx = chk(cu.cuDevicePrimaryCtxRetain(dev))
        chk(cu.cuCtxSetCurrent(ctx))
        maj = chk(cu.cuDeviceGetAttribute(cu.CUdevice_attribute.CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR, dev))
        mi = chk(cu.cuDeviceGetAttribute(cu.CUdevice_attribute.CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR, dev))
    except Exception as ex:  # noqa: BLE001
        return None, f"no usable CUDA device ({ex})"
    return (cu, nvrtc, chk, f"sm_{maj}{mi}"), ""


def gpu_launch(tools, module, fname, arrays_in, n_out, n):
    cu, nvrtc, chk, _ = tools
    f = chk(cu.cuModuleGetFunction(module, fname.encode()))
    bufs, outs = [], [np.full(n, np.nan) for _ in range(n_out)]
    for a in list(arrays_in) + outs:
        p = chk(cu.cuMemAlloc(a.nbytes))
        chk(cu.cuMemcpyHtoD(p, a.ctypes.data, a.nbytes))
        bufs.append(p)
    vals = [np.array([int(p)], dtype=np.uint64) for p in bufs] + [np.array([n], dtype=np.uint32)]
    argp = np.array([a.ctypes.data for a in vals], dtype=np.uint64)
    chk(cu.cuLaunchKernel(f, (n + 63) // 64, 1, 1, 64, 1, 1, 0, 0, argp.ctypes.data, 0))
    chk(cu.cuCtxSynchronize())
    for p, a in zip(bufs[len(arrays_in):], outs):
        chk(cu.cuMemcpyDtoH(a.ctypes.data, p, a.nbytes))
    for p in bufs:
        chk(cu.cuMemFree(p))
    return outs


def check_gpu(rep, args, tmp, kernels, exes):
    tools, why = gpu_tools()
    if tools is None:
        rep.add("codegen/cuda-tile/gpu", "UNVERIFIED", why)
        rep.add("codegen/cuda-ptx/dynamic", "UNVERIFIED", why)
        return
    cu, nvrtc, chk, arch = tools
    for model, (ins, pars, outs, fn, tile_fn) in MODELS.items():
        src = kernels[(model, "cuda-tile")].encode()
        prog = chk(nvrtc.nvrtcCreateProgram(src, b"k.cu", 0, [], []))
        r = nvrtc.nvrtcCompileProgram(prog, 1, [f"--gpu-architecture={arch}".encode()])
        if int(r[0]) != 0:
            n = chk(nvrtc.nvrtcGetProgramLogSize(prog))
            buf = b" " * n
            nvrtc.nvrtcGetProgramLog(prog, buf)
            rep.add(f"codegen/{model}/cuda-tile/nvrtc", "FAIL", buf.decode()[:300])
            continue
        n = chk(nvrtc.nvrtcGetPTXSize(prog))
        ptx = b" " * n
        chk(nvrtc.nvrtcGetPTX(prog, ptx))
        mod = chk(cu.cuModuleLoadData(ptx))
        pts = grid(model)
        cols = np.array([i + p for i, p in pts], dtype=np.float64).T.copy()
        got = gpu_launch(tools, mod, tile_fn, [c.copy() for c in cols], len(outs), len(pts))
        cpu = np.array(kernel_eval(exes[model], [i + p for i, p in pts])).T
        worst = 0.0
        for k in range(len(outs)):
            for a, b in zip(got[k], cpu[k]):
                worst = max(worst, abs(a - b) / max(abs(b), 1e-30 if abs(b) > 0 else 1.0, 1e-300) if abs(b) > 1e-300 else abs(a))
        rep.add(f"codegen/{model}/cuda-tile/gpu_vs_rust", "PASS" if worst <= TILE_REL else "FAIL", f"{len(pts)} points x {len(outs)} outputs on the GPU ({arch}, fp64) vs compiled-Rust kernel: worst rel {worst:.1e} (gate {TILE_REL:g})")
    # PTX: an empty kernel
    for model, (ins, pars, outs, fn, tile_fn) in MODELS.items():
        ptx = kernels[(model, "cuda-ptx")]
        entry = re.search(r"\.entry (\w+)\(", ptx).group(1)
        try:
            mod = chk(cu.cuModuleLoadData(ptx.encode() + b"\0"))
        except Exception as ex:  # noqa: BLE001
            rep.add(f"codegen/{model}/cuda-ptx/dynamic", "UNVERIFIED", f"module load failed: {ex}")
            continue
        pts = grid(model)[:8]
        cols = np.array([i + p for i, p in pts], dtype=np.float64).T.copy()
        got = gpu_launch(tools, mod, entry, [c.copy() for c in cols], len(outs), len(pts))
        untouched = all(bool(np.isnan(o).all()) for o in got)
        rep.add(f"gap/codegen-ptx-empty-kernel/{model}/dynamic", "KNOWN_GAP_CONFIRMED" if untouched else "GAP_CLOSED", f"{entry} launched on the GPU: {sum(bool(np.isnan(o).all()) for o in got)}/{len(outs)} output buffers still hold the NaN sentinel" if untouched else "kernel now writes outputs: validate them against the rust kernel")


# ------------------------------------------------------------------------------- static structure of all targets
OPS = {"+": "add", "-": "sub", "*": "mul", "/": "div"}


def ssa_rust(code, model):
    ins, pars, _, _, _ = MODELS[model]
    out = {}
    for m in re.finditer(r"let v(\d+): f64 = (.+);", code):
        i, e = int(m.group(1)), m.group(2).strip()
        if re.fullmatch(r"inputs\[(\d+)\]", e):
            out[i] = ("in", ins[int(re.fullmatch(r"inputs\[(\d+)\]", e).group(1))])
        elif re.fullmatch(r"params\[(\d+)\]", e):
            out[i] = ("par", pars[int(re.fullmatch(r"params\[(\d+)\]", e).group(1))])
        elif re.fullmatch(r"[-+0-9.e]+_f64", e):
            out[i] = ("const", float(e[:-4]))
        elif (mm := re.fullmatch(r"-v(\d+)", e)):
            out[i] = ("neg", int(mm.group(1)))
        elif (mm := re.fullmatch(r"v(\d+)\.clamp\(-80\.0, 80\.0\)\.exp\(\)", e)):
            out[i] = ("exp", int(mm.group(1)))
        elif (mm := re.fullmatch(r"v(\d+)\.max\(1e-30\)\.ln\(\)", e)):
            out[i] = ("ln", int(mm.group(1)))
        elif (mm := re.fullmatch(r"v(\d+) ([-+*/]) v(\d+)", e)):
            out[i] = (OPS[mm.group(2)], int(mm.group(1)), int(mm.group(3)))
        elif (mm := re.fullmatch(r"if v(\d+) > v(\d+) \{ 1\.0 \} else \{ 0\.0 \}", e)):
            out[i] = ("gt", int(mm.group(1)), int(mm.group(2)))
        elif (mm := re.fullmatch(r"if v(\d+) > 0\.0 \{ v(\d+) \} else \{ v(\d+) \}", e)):
            out[i] = ("select", int(mm.group(1)), int(mm.group(2)), int(mm.group(3)))
        else:
            out[i] = ("?", e)
    return out


def ssa_other(code, model, target):
    ins, pars, _, _, _ = MODELS[model]
    names = {}
    for n in ins:
        names["in_" + n] = ("in", n)
    for n in pars:
        names["param_" + n] = ("par", n)
    out = {}
    pat = {"cuda-tile": r"const double v(\d+) = (.+);", "msl": r"df64 v(\d+) = (.+);", "wgsl": r"let v(\d+) = (.+);"}[target]
    for m in re.finditer(pat, code):
        i, e = int(m.group(1)), m.group(2).strip()
        if (mm := re.fullmatch(r"(\w+)\[gid\]", e)):
            out[i] = names.get(mm.group(1), ("?", e))
            continue
        if target == "cuda-tile":
            if (mm := re.fullmatch(r"[-+0-9.e]+", e)):
                out[i] = ("const", float(e))
            elif (mm := re.fullmatch(r"\(v(\d+) ([-+*/]) v(\d+)\)", e)):
                out[i] = (OPS[mm.group(2)], int(mm.group(1)), int(mm.group(3)))
            elif (mm := re.fullmatch(r"-v(\d+)", e)):
                out[i] = ("neg", int(mm.group(1)))
            elif (mm := re.fullmatch(r"exp\(fmin\(fmax\(v(\d+), -80\.0\), 80\.0\)\)", e)):
                out[i] = ("exp", int(mm.group(1)))
            elif (mm := re.fullmatch(r"log\(fmax\(v(\d+), 1e-30\)\)", e)):
                out[i] = ("ln", int(mm.group(1)))
            elif (mm := re.fullmatch(r"\(\(v(\d+) > v(\d+)\) \? 1\.0 : 0\.0\)", e)):
                out[i] = ("gt", int(mm.group(1)), int(mm.group(2)))
            elif (mm := re.fullmatch(r"\(v(\d+) > 0\.0\) \? v(\d+) : v(\d+)", e)):
                out[i] = ("select", int(mm.group(1)), int(mm.group(2)), int(mm.group(3)))
            else:
                out[i] = ("?", e)
        elif target == "msl":
            if (mm := re.fullmatch(r"df64\{([-+0-9.e]+)f, ([-+0-9.e]+)f\}", e)):
                out[i] = ("const", float(mm.group(1)) + float(mm.group(2)))
            elif (mm := re.fullmatch(r"df64_(add|sub|mul|div)\(v(\d+), v(\d+)\)", e)):
                out[i] = (mm.group(1), int(mm.group(2)), int(mm.group(3)))
            elif (mm := re.fullmatch(r"df64\{-v(\d+)\.hi, -v\1\.lo\}", e)):
                out[i] = ("neg", int(mm.group(1)))
            elif (mm := re.fullmatch(r"df64_(exp|ln)\(v(\d+)\)", e)):
                out[i] = (mm.group(1), int(mm.group(2)))
            elif (mm := re.fullmatch(r"\(df64_sub\(v(\d+), v(\d+)\)\.hi > 0\.0f \? df64\{1\.0f, 0\.0f\} : df64\{0\.0f, 0\.0f\}\)", e)):
                out[i] = ("gt", int(mm.group(1)), int(mm.group(2)))
            elif (mm := re.fullmatch(r"df64_select\(v(\d+), v(\d+), v(\d+)\)", e)):
                out[i] = ("select", int(mm.group(1)), int(mm.group(2)), int(mm.group(3)))
            else:
                out[i] = ("?", e)
        else:  # wgsl
            if (mm := re.fullmatch(r"vec2<f32>\(([-+0-9.e]+), ([-+0-9.e]+)\)", e)):
                out[i] = ("const", float(mm.group(1)) + float(mm.group(2)))
            elif (mm := re.fullmatch(r"df64_(add|sub|mul|div)\(v(\d+), v(\d+)\)", e)):
                out[i] = (mm.group(1), int(mm.group(2)), int(mm.group(3)))
            elif (mm := re.fullmatch(r"vec2<f32>\(-v(\d+)\.x, -v\1\.y\)", e)):
                out[i] = ("neg", int(mm.group(1)))
            elif (mm := re.fullmatch(r"df64_(exp|ln)\(v(\d+)\)", e)):
                out[i] = (mm.group(1), int(mm.group(2)))
            elif (mm := re.fullmatch(r"select\(vec2<f32>\(0\.0, 0\.0\), vec2<f32>\(1\.0, 0\.0\), df64_sub\(v(\d+), v(\d+)\)\.x > 0\.0\)", e)):
                out[i] = ("gt", int(mm.group(1)), int(mm.group(2)))
            elif (mm := re.fullmatch(r"select\(v(\d+), v(\d+), v(\d+)\.x > 0\.0\)", e)):
                out[i] = ("select", int(mm.group(3)), int(mm.group(2)), int(mm.group(1)))  # select(f, t, cond)
            else:
                out[i] = ("?", e)
    return out


def same(a, b):
    if a[0] != b[0] or len(a) != len(b):
        return False
    if a[0] == "const":
        return abs(a[1] - b[1]) <= 1e-6 * max(1.0, abs(a[1]))
    return a[1:] == b[1:]


def outputs_of(code, target, model):
    outs = MODELS[model][2]
    if target == "rust":
        return {o: int(m) for m, o in re.findall(r"outputs\[\d+\] = v(\d+); // (\w+)", code)}
    return {o: int(v) for o, v in re.findall(r"out_(\w+)\[gid\] = v(\d+);", code)}


def check_structure(rep, kernels):
    for model, (ins, pars, outs, fn, _) in MODELS.items():
        ref = ssa_rust(kernels[(model, "rust")], model)
        unparsed = [k for k, v in ref.items() if v[0] == "?"]
        ref_out = outputs_of(kernels[(model, "rust")], "rust", model)
        rep.add(f"structure/{model}/rust_parsed", "PASS" if not unparsed and list(ref_out) == outs else "FAIL", f"{len(ref)} SSA statements, outputs {list(ref_out)}")
        for tgt in ("cuda-tile", "msl", "wgsl"):
            code = kernels[(model, tgt)]
            o = ssa_other(code, model, tgt)
            diffs = [i for i in ref if i not in o or not same(ref[i], o[i])]
            diffs += [i for i in o if i not in ref]
            bal = code.count("{") == code.count("}") and code.count("(") == code.count(")")
            out_ok = outputs_of(code, tgt, model) == ref_out
            bad = "nan" in code.lower().replace("nan_", "") or re.search(r"\binf\b", code.lower())
            ok = not diffs and bal and out_ok and not bad
            rep.add(f"structure/{model}/{tgt}_equals_rust", "PASS" if ok else "FAIL", f"{len(o)} SSA statements, {len(diffs)} differ (first {diffs[:3]}), outputs match={out_ok}, balanced={bal}")
        ptx = kernels[(model, "cuda-ptx")]
        body = ptx[ptx.index(") {") :]
        n_store = len(re.findall(r"\bst\.\w+", body))
        n_params = len(re.findall(r"\.param \.u64 param_out_", ptx))
        gap = n_store < n_params
        rep.add(f"gap/codegen-ptx-empty-kernel/{model}/static", "KNOWN_GAP_CONFIRMED" if gap else "GAP_CLOSED", f"PTX declares {n_params} output pointers but contains {n_store} store instructions (body: {' '.join(body.split())[:80]!r})" if gap else f"{n_store} stores for {n_params} outputs")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default=DEFAULT_BIN)
    ap.add_argument("--ngspice", default=DEFAULT_NGSPICE)
    ap.add_argument("--rustc", default=shutil.which("rustc") or "rustc")
    ap.add_argument("--decks", default=DECK_DIR, help="directory with the cap_cli_* fixture decks (default: benchmarks/cases/capability)")
    ap.add_argument("--cpus", default=None)
    ap.add_argument("--json", default=None)
    ap.add_argument("--inject-error", type=float, default=0.0, help="self-test: scale output #3 of every kernel evaluation by (1+x); the gates must then fail")
    ap.add_argument("--no-gpu", action="store_true")
    ap.add_argument("--strict", action="store_true")
    args = ap.parse_args()
    pin_cpus(args.cpus)
    INJECT["rel"] = args.inject_error
    if not os.access(args.bin, os.X_OK) or shutil.which(args.rustc) is None or shutil.which(args.ngspice) is None:
        print("unverified: rustspice, rustc or ngspice not available")
        return 2
    rep, tmp = Report(), Tmp()
    try:
        kernels = {}
        for model in MODELS:
            for tgt in ("rust", "cuda-tile", "cuda-ptx", "msl", "wgsl"):
                rc, out, err, _ = run([args.bin, "codegen", "--model", model, "--target", tgt])
                if rc != 0 or not out.strip():
                    rep.add(f"codegen/{model}/{tgt}/emit", "FAIL", f"rc={rc} {err[-120:]}")
                kernels[(model, tgt)] = out
        exes = check_rust(rep, args, tmp, kernels)
        if len(exes) == len(MODELS):
            check_netlists(rep, args, tmp, exes)
            if not args.no_gpu:
                check_gpu(rep, args, tmp, kernels, exes)
        check_structure(rep, kernels)
    finally:
        tmp.close()
    gaps = sum(1 for r in rep.rows if r["status"] == "KNOWN_GAP_CONFIRMED")
    print(f"\nsummary: {len(rep.rows)} checks, {rep.bad} unexpected failures, {gaps} known gaps confirmed")
    if args.json:
        json.dump({"script": "check_codegen_kernels", "bad": rep.bad, "rows": rep.rows}, open(args.json, "w"), indent=1)
    return 1 if rep.bad or (args.strict and gaps) else 0


if __name__ == "__main__":
    sys.exit(main())
