//! Cloud-Native WebAssembly (`wasm32-unknown-unknown`) SIMD128 + WebGPU (`WGSL` `df64`)
//! Compute Backend (`WasmWebGpuBackend`).
//!
//! Dispatches small-to-medium circuits to WASM `simd128` (`f64x2`) and large circuits
//! to the WebGPU `df64` (`vec2<f32>`) compute shader pipeline.

use rustspice_backend_cpu::CpuSimdBackend;
use rustspice_backend_metal::MetalUmaBackend;
use rustspice_core::{
    intrinsic_diode_kernel_wgsl_shader, intrinsic_mosfet_kernel_wgsl_shader, ComputeBackend,
    CscMatrix, DeviceSoaBuffer, SimResult,
};
use rustspice_ir::{
    build_diode_ir, build_mosfet_ekv_ir, build_mosfet_level1_ir, emit_wgsl_df64_kernel,
};

/// Bundle of WebGPU Shading Language (`WGSL`) `df64` compute shaders for browser execution.
#[derive(Clone, Debug)]
pub struct WgslShaderBundle {
    pub diode_wgsl: String,
    pub mosfet_level1_wgsl: String,
    pub mosfet_ekv_wgsl: String,
    pub proc_macro_diode_wgsl: &'static str,
    pub proc_macro_mosfet_wgsl: &'static str,
}

impl Default for WgslShaderBundle {
    fn default() -> Self {
        Self::new()
    }
}

impl WgslShaderBundle {
    pub fn new() -> Self {
        Self {
            diode_wgsl: emit_wgsl_df64_kernel(&build_diode_ir()),
            mosfet_level1_wgsl: emit_wgsl_df64_kernel(&build_mosfet_level1_ir()),
            mosfet_ekv_wgsl: emit_wgsl_df64_kernel(&build_mosfet_ekv_ir()),
            proc_macro_diode_wgsl: intrinsic_diode_kernel_wgsl_shader(),
            proc_macro_mosfet_wgsl: intrinsic_mosfet_kernel_wgsl_shader(),
        }
    }
}

/// Hybrid WASM SIMD128 + WebGPU `df64` Backend.
#[derive(Clone, Debug)]
pub struct WasmWebGpuBackend {
    pub webgpu_threshold: usize,
    pub shaders: WgslShaderBundle,
    simd_backend: CpuSimdBackend,
    df64_backend: MetalUmaBackend,
}

impl Default for WasmWebGpuBackend {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl WasmWebGpuBackend {
    pub fn new(webgpu_threshold: usize) -> Self {
        Self {
            webgpu_threshold,
            shaders: WgslShaderBundle::new(),
            simd_backend: CpuSimdBackend::new(),
            df64_backend: MetalUmaBackend::new(),
        }
    }
}

impl ComputeBackend for WasmWebGpuBackend {
    fn backend_name(&self) -> &'static str {
        "wasm-simd128-webgpu-df64"
    }

    fn evaluate_devices(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) -> SimResult<()> {
        let total_nl = state.diode_names.len() + state.mos_names.len();
        if total_nl >= self.webgpu_threshold {
            self.df64_backend.evaluate_devices(state, voltages)
        } else {
            self.simd_backend.evaluate_devices(state, voltages)
        }
    }

    fn stamp_jacobian(
        &self,
        state: &DeviceSoaBuffer,
        jacobian: &mut CscMatrix<f64>,
        rhs: &mut [f64],
    ) -> SimResult<()> {
        self.simd_backend.stamp_jacobian(state, jacobian, rhs)
    }
}
