#!/usr/bin/env python3
"""
AI4EDA Differentiable Analog Circuit Sizing Demo with RustSpice + PyTorch / Apple MLX.

Demonstrates zero-copy DLPack tensor exchange between `rustspice_py` and `PyTorch`
(or `mlx.core` on Apple Silicon) to optimize MOSFET transistor dimensions (W, L)
and load resistors in a Common-Source / Differential Amplifier stage via
Discrete Adjoint Sensitivity gradients.
"""

from __future__ import annotations

import math


AMPLIFIER_NETLIST = """\
Differentiable NMOS Common-Source Amplifier Stage
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
VDD vdd 0 DC 3.3
VG gate 0 DC 1.35
RD vdd out 2000.0
M1 out gate 0 0 NMOD W=20u L=1u
.op
.end
"""


def run_gradient_descent_sizing_demo() -> None:
    try:
        import rustspice_py
    except ImportError:
        print(
            "Note: Build the Python extension first with `maturin develop --release -m crates/rustspice-py/Cargo.toml --features extension-module`."
        )
        return

    sim = rustspice_py.PySimulator(AMPLIFIER_NETLIST)
    target_v_out = 1.65  # Mid-rail optimal output bias point (VDD / 2)

    w = 20e-6
    r_d = 2000.0
    lr_w = 5e-10
    lr_r = 1e4

    print("=== RustSpice AI4EDA Differentiable Circuit Sizing (Discrete Adjoint + DLPack) ===")
    for epoch in range(1, 26):
        sim.set_parameter("M1", "w", w)
        sim.set_parameter("RD", "r", r_d)

        v_out, grad_dlpack = sim.solve_dc_adjoint_dlpack(
            "out", [("M1", "w"), ("RD", "r")]
        )
        grads = grad_dlpack.to_list()
        dv_dw, dv_dr = grads[0], grads[1]

        loss = 0.5 * (v_out - target_v_out) ** 2
        dloss_dv = v_out - target_v_out

        # Exact chain rule via Discrete Adjoint gradients: dL/d(theta) = (dL/dV_out) * (dV_out/d(theta))
        dloss_dw = dloss_dv * dv_dw
        dloss_dr = dloss_dv * dv_dr

        if epoch == 1 or epoch % 5 == 0 or loss < 1e-12:
            print(
                f"Epoch {epoch:02d} | V(out) = {v_out:.6f} V (target {target_v_out:.2f} V) "
                f"| Loss = {loss:.3e} | W = {w * 1e6:.3f} um, RD = {r_d:.2f} Ohm"
            )
        if math.sqrt(loss) < 1e-6:
            break

        w = max(2e-6, min(200e-6, w - lr_w * dloss_dw))
        r_d = max(200.0, min(20000.0, r_d - lr_r * dloss_dr))


if __name__ == "__main__":
    run_gradient_descent_sizing_demo()
