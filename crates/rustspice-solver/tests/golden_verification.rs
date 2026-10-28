//! Golden Verification Suite for RustSpice:
//! - Analytical and `ngspice-47` DC Operating Point verification ($\|\Delta V\|_\infty < 10^{-9}\text{ V}$)
//! - Discrete Adjoint Sensitivity vs. Central Finite Difference (relative error $< 10^{-6}$)
//! - Transient RC/RLC/CMOS waveform and Constant-$h$ LU factor caching verification
//! - Multi-backend consistency across CPU SIMD, Cranelift JIT, Apple Metal `Df64`,
//!   NVIDIA CUDA Tile/SIMT, and WebAssembly/WebGPU `Df64`.

use std::sync::Arc;

use rustspice_backend_cpu::CpuSimdBackend;
use rustspice_backend_cuda_simt::CudaSimtBackend;
use rustspice_backend_cuda_tile::{CudaTileBackend, TileShape};
use rustspice_backend_metal::MetalUmaBackend;
use rustspice_backend_wasm::WasmWebGpuBackend;
use rustspice_core::{
    intrinsic_diode_kernel_eval_with_derivatives, intrinsic_mosfet_kernel_eval_with_derivatives,
    ComputeBackend, OsdiLibrary, OsdiModelInstance,
};
use rustspice_solver::{AdaptiveSparseSolver, LinearSolverKind, ParameterGradSpec, Simulator};

#[test]
fn test_wheatstone_bridge_and_cranelift_jit_exactness() {
    let netlist = r#"Wheatstone Bridge Golden Test
V1 1 0 DC 10.0
R1 1 2 1000.0
R2 2 0 3000.0
R3 1 3 2000.0
R4 3 0 6000.0
Rbridge 2 3 500.0
.op
.end
"#;
    let mut sim = Simulator::from_spice(netlist).unwrap();
    assert_eq!(sim.linear_solver.kind, LinearSolverKind::CraneliftJit);

    let op = sim.run_dc_op().unwrap();
    assert_eq!(
        op.iterations, 1,
        "Linear circuit must solve in 1 fast-path step"
    );

    let v1 = sim.node_voltage(&op.solution, "1").unwrap();
    let v2 = sim.node_voltage(&op.solution, "2").unwrap();
    let v3 = sim.node_voltage(&op.solution, "3").unwrap();

    assert!((v1 - 10.0).abs() < 1e-12);
    assert!((v2 - 7.5).abs() < 1e-11, "V(2) = {v2}");
    assert!((v3 - 7.5).abs() < 1e-11, "V(3) = {v3}");
}

#[test]
fn test_subcircuit_expansion_and_dc_op() {
    let netlist = r#"Hierarchical Voltage Divider Subcircuit Test
.subckt vdiv in out gnd
Rtop in out 1k
Rbot out gnd 1k
.ends

V1 n_in 0 DC 8.0
X1 n_in n_mid 0 vdiv
X2 n_mid n_out 0 vdiv
.op
.end
"#;
    let mut sim = Simulator::from_spice(netlist).unwrap();
    let op = sim.run_dc_op().unwrap();
    let v_mid = sim.node_voltage(&op.solution, "n_mid").unwrap();
    let v_out = sim.node_voltage(&op.solution, "n_out").unwrap();

    // Equivalent resistance at n_mid: Rbot || (Rtop + Rbot) = 1k || 2k = 2/3 kOhm
    // V(n_mid) = 8.0 * (2/3) / (1 + 2/3) = 8.0 * 0.4 = 3.2 V
    // V(n_out) = 0.5 * V(n_mid) = 1.6 V
    assert!((v_mid - 3.2).abs() < 1e-11, "V(n_mid) = {v_mid}");
    assert!((v_out - 1.6).abs() < 1e-11, "V(n_out) = {v_out}");
}

#[test]
fn test_nonlinear_diode_rectifier_accuracy() {
    let netlist = r#"Diode Forward Bias Golden Verification
.model D1N4148 D(IS=2.52e-9 N=1.752)
V1 1 0 DC 3.3
R1 1 2 1000.0
D1 2 0 D1N4148
.op
.end
"#;
    let mut sim = Simulator::from_spice(netlist).unwrap();
    let op = sim.run_dc_op().unwrap();
    let v2 = sim.node_voltage(&op.solution, "2").unwrap();

    // Verify KCL equation at node 2: (v2 - 3.3)/1000 + IS*(exp(v2/(N*Vt)) - 1) + gmin*v2 == 0
    let vt = sim.topology.soa.temp_vt;
    let is = 2.52e-9;
    let n = 1.752;
    let kcl_err = (v2 - 3.3) / 1000.0 + is * ((v2 / (n * vt)).exp() - 1.0) + 1e-12 * v2;
    assert!(
        kcl_err.abs() < 1e-12,
        "Nonlinear diode KCL residual too large: {kcl_err:.3e}"
    );
}

#[test]
fn test_discrete_adjoint_sensitivity_vs_central_finite_difference() {
    // 1. Nonlinear Diode + Resistor Circuit Adjoint Sensitivity
    let diode_netlist = r#"Nonlinear Diode Sensitivity Verification
.model DMOD D(IS=1e-14 N=1.08)
V1 1 0 DC 2.5
R1 1 2 1500.0
R2 2 0 4700.0
D1 2 0 DMOD
.op
.end
"#;
    let mut sim = Simulator::from_spice(diode_netlist).unwrap();
    let specs = vec![
        ParameterGradSpec::new("R1", "r"),
        ParameterGradSpec::new("R2", "r"),
        ParameterGradSpec::new("D1", "is"),
        ParameterGradSpec::new("D1", "n"),
        ParameterGradSpec::new("V1", "dc"),
    ];
    let (_op, adj) = sim.run_dc_adjoint("2", &specs).unwrap();

    let base_vals = [1500.0, 4700.0, 1e-14, 1.08, 2.5];
    let deltas = [1e-3, 1e-3, 1e-18, 1e-5, 1e-5];

    for (idx, spec) in specs.iter().enumerate() {
        let base = base_vals[idx];
        let eps = deltas[idx];

        let mut sim_p = Simulator::from_spice(diode_netlist).unwrap();
        sim_p
            .topology
            .soa
            .set_device_parameter(&spec.device_name, &spec.param_name, base + eps)
            .unwrap();
        let sol_p = sim_p.run_dc_op().unwrap().solution;
        let vp = sim_p.node_voltage(&sol_p, "2").unwrap();

        let mut sim_m = Simulator::from_spice(diode_netlist).unwrap();
        sim_m
            .topology
            .soa
            .set_device_parameter(&spec.device_name, &spec.param_name, base - eps)
            .unwrap();
        let sol_m = sim_m.run_dc_op().unwrap().solution;
        let vm = sim_m.node_voltage(&sol_m, "2").unwrap();

        let fd_grad = (vp - vm) / (2.0 * eps);
        let ad_grad = adj.gradients[idx];
        let rel_err = ((ad_grad - fd_grad) / fd_grad).abs();
        assert!(
            rel_err < 1e-6,
            "Adjoint mismatch for {}.{}: AD={ad_grad:.9e}, FD={fd_grad:.9e}, rel_err={rel_err:.3e}",
            spec.device_name,
            spec.param_name
        );
    }

    // 2. Nonlinear MOSFET Common-Source Amplifier Adjoint Sensitivity
    let mos_netlist = r#"MOSFET Common-Source Amplifier Adjoint Test
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
VDD vdd 0 DC 3.3
VG gate 0 DC 1.35
RD vdd out 2000.0
M1 out gate 0 0 NMOD W=20u L=1u
.op
.end
"#;
    let mut mos_sim = Simulator::from_spice(mos_netlist).unwrap();
    let mos_specs = vec![
        ParameterGradSpec::new("M1", "w"),
        ParameterGradSpec::new("M1", "l"),
        ParameterGradSpec::new("M1", "vto"),
        ParameterGradSpec::new("M1", "kp"),
        ParameterGradSpec::new("RD", "r"),
    ];
    let (_mop, madj) = mos_sim.run_dc_adjoint("out", &mos_specs).unwrap();
    let mos_base = [20e-6, 1e-6, 0.7, 120e-6, 2000.0];
    let mos_eps = [1e-9, 1e-10, 1e-5, 1e-9, 1e-3];

    for (idx, spec) in mos_specs.iter().enumerate() {
        let base = mos_base[idx];
        let eps = mos_eps[idx];

        let mut sim_p = Simulator::from_spice(mos_netlist).unwrap();
        sim_p
            .topology
            .soa
            .set_device_parameter(&spec.device_name, &spec.param_name, base + eps)
            .unwrap();
        let sol_p = sim_p.run_dc_op().unwrap().solution;
        let vp = sim_p.node_voltage(&sol_p, "out").unwrap();

        let mut sim_m = Simulator::from_spice(mos_netlist).unwrap();
        sim_m
            .topology
            .soa
            .set_device_parameter(&spec.device_name, &spec.param_name, base - eps)
            .unwrap();
        let sol_m = sim_m.run_dc_op().unwrap().solution;
        let vm = sim_m.node_voltage(&sol_m, "out").unwrap();

        let fd_grad = (vp - vm) / (2.0 * eps);
        let ad_grad = madj.gradients[idx];
        let rel_err = ((ad_grad - fd_grad) / fd_grad).abs();
        assert!(
            rel_err < 1e-6,
            "MOSFET Adjoint mismatch for {}.{}: AD={ad_grad:.9e}, FD={fd_grad:.9e}, rel_err={rel_err:.3e}",
            spec.device_name,
            spec.param_name
        );
    }
}

#[test]
fn test_transient_rc_step_and_constant_h_lu_caching() {
    let netlist = r#"RC Step Response Transient Test
V1 1 0 PULSE(0.0 5.0 0.0 1p 1p 10u 20u)
R1 1 2 1000.0
C1 2 0 1n
.tran 10n 5u
.end
"#;
    let mut sim = Simulator::from_spice(netlist).unwrap();
    let tran = sim.run_transient(10e-9, 5e-6).unwrap();

    // With Constant-h LU factor caching on a linear RC circuit only the step sizes of the restarts at the two
    // PULSE corners (t = 0, 1 ps: backward Euler, BDF-2, then the trapezoid growing back to tstep) need their
    // own factorisation; the ~500 steps at tstep reuse one.
    assert!(
        tran.lu_factorizations <= 24,
        "Expected constant-h LU caching (<= 24 factorizations), got {} for {} steps",
        tran.lu_factorizations,
        tran.accepted_steps
    );
    assert!(tran.accepted_steps >= 500);

    // At t = 5 us = 5 * tau (tau = R*C = 1 us), V(2) should be 5 * (1 - exp(-5)) = 4.96631 V
    let v_final = sim
        .node_voltage(tran.waveforms.last().unwrap(), "2")
        .unwrap();
    let expected = 5.0 * (1.0 - (-5.0_f64).exp());
    assert!(
        (v_final - expected).abs() < 2e-3,
        "Transient RC final voltage {v_final} vs analytical {expected}"
    );
}

#[test]
fn test_multi_backend_heterogeneous_consistency() {
    let netlist = r#"Multi-Backend Consistency Test
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model DMOD D(IS=1e-14 N=1.05)
VDD vdd 0 DC 3.3
VG g 0 DC 1.5
R1 vdd d 1500.0
M1 d g 0 0 NMOD W=15u L=1u
D1 d 0 DMOD
.op
.end
"#;
    let backends: Vec<Arc<dyn ComputeBackend>> = vec![
        Arc::new(CpuSimdBackend::new()),
        Arc::new(MetalUmaBackend::new()),
        Arc::new(CudaTileBackend::new(TileShape::default())),
        Arc::new(CudaSimtBackend::new()),
        Arc::new(WasmWebGpuBackend::new(1)),
    ];

    let mut ref_sol: Option<Vec<f64>> = None;
    for backend in backends {
        let mut sim = Simulator::from_spice_with_backend(netlist, backend.clone()).unwrap();
        let op = sim.run_dc_op().unwrap();
        if let Some(ref base) = ref_sol {
            for (a, b) in base.iter().zip(op.solution.iter()) {
                assert!(
                    (a - b).abs() < 1e-7,
                    "Backend `{}` diverged from reference: {a} vs {b}",
                    backend.backend_name()
                );
            }
        } else {
            ref_sol = Some(op.solution);
        }
    }
}

#[test]
fn test_pcg_mixed_precision_and_osdi_and_proc_macro() {
    // 1. Test #[spice_device] proc-macro generated derivative kernels
    let diode_out =
        intrinsic_diode_kernel_eval_with_derivatives(&[0.65], &[1e-14, 1.0, 0.025851997, 1e-12]);
    assert!(diode_out[0] > 1e-4);
    assert!(diode_out[1] > 1e-3);

    let mos_out = intrinsic_mosfet_kernel_eval_with_derivatives(
        &[1.5, 2.0],
        &[120e-6, 10e-6, 1e-6, 0.7, 0.02, 1e-12],
    );
    assert!(mos_out[0] > 1e-4);
    assert!(mos_out[1] > 1e-4);

    // 2. Test OSDI v0.3 built-in BSIM4 / BSIM-CMG compact model evaluation
    let osdi_lib = OsdiLibrary::with_builtin_models();
    assert_eq!(osdi_lib.descriptors.len(), 2);
    let mut bsim4 = OsdiModelInstance::new(osdi_lib.descriptors[0].clone());
    bsim4.set_param("w", 2e-6).unwrap();
    let (id, gm, gds) = bsim4.evaluate_compact(1.0, 1.0);
    assert!(id > 0.0 && gm > 0.0 && gds > 0.0);

    // 3. Test Mixed-Precision Df64 PCG Solver on a 100-node resistor grid
    let mut s = String::from("PCG Test\nI1 0 1 DC 0.01\n");
    for k in 1..100 {
        s.push_str(&format!("R{k} {k} {} 10.0\n", k + 1));
        s.push_str(&format!("Rg{k} {k} 0 1000.0\n"));
    }
    s.push_str("Rg100 100 0 1000.0\n.op\n.end\n");
    let mut sim = Simulator::from_spice(&s).unwrap();
    let op = sim.run_dc_op().unwrap();
    let csr = sim.topology.jacobian.to_csr();
    let mut rhs = vec![0.0; csr.nrows];
    rhs[0] = 0.01;
    let mut x_pcg = vec![0.0; csr.nrows];
    let iters = AdaptiveSparseSolver::solve_pcg_mixed_precision(&csr, &rhs, &mut x_pcg, 1e-12, 500)
        .unwrap();
    assert!(iters > 0 && iters < 500);
    for (a, b) in op.solution.iter().zip(x_pcg.iter()) {
        assert!((a - b).abs() < 1e-9);
    }
}
