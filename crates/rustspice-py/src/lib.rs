//! Zero-Copy `DLPack` and `PyO3` Bridge for AI4EDA (`PyTorch` & Apple `MLX`).
//!
//! Exposes simulation waveforms, DC operating points, and Discrete Adjoint parameter
//! gradients as zero-copy [`DLManagedTensor`] capsules implementing the standard Python
//! `__dlpack__` and `__dlpack_device__` protocols.

use std::ffi::{c_void, CString};
use std::ptr;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use rustspice_solver::{ParameterGradSpec, Simulator};

/// DLPack device type code (`kDLCPU = 1`, `kDLCUDA = 2`, `kDLMetal = 8`).
pub const K_DL_CPU: i32 = 1;
/// DLPack data type code (`kDLFloat = 2`).
pub const K_DL_FLOAT: u8 = 2;

/// Standard C-ABI `DLDevice` descriptor from the DLPack specification.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DLDevice {
    pub device_type: i32,
    pub device_id: i32,
}

/// Standard C-ABI `DLDataType` descriptor from the DLPack specification.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DLDataType {
    pub code: u8,
    pub bits: u8,
    pub lanes: u16,
}

/// Standard C-ABI `DLTensor` structure describing an N-dimensional array view.
#[repr(C)]
#[derive(Debug)]
pub struct DLTensor {
    pub data: *mut c_void,
    pub device: DLDevice,
    pub ndim: i32,
    pub dtype: DLDataType,
    pub shape: *mut i64,
    pub strides: *mut i64,
    pub byte_offset: u64,
}

/// Standard C-ABI `DLManagedTensor` with custom ownership context and destructor.
#[repr(C)]
pub struct DLManagedTensor {
    pub dl_tensor: DLTensor,
    pub manager_ctx: *mut c_void,
    pub deleter: Option<unsafe extern "C" fn(*mut DLManagedTensor)>,
}

struct OwnedDlPackContext {
    _data: Vec<f64>,
    _shape: Vec<i64>,
    _strides: Vec<i64>,
}

unsafe extern "C" fn dlpack_managed_deleter(managed_ptr: *mut DLManagedTensor) {
    if managed_ptr.is_null() {
        return;
    }
    unsafe {
        let boxed_tensor = Box::from_raw(managed_ptr);
        if !boxed_tensor.manager_ctx.is_null() {
            let _ = Box::from_raw(boxed_tensor.manager_ctx as *mut OwnedDlPackContext);
        }
    }
}

/// Allocates a heap-managed `DLManagedTensor` wrapping an owned `Vec<f64>` with zero data copy.
pub fn into_dlpack_managed_tensor(mut data: Vec<f64>, shape: Vec<i64>) -> *mut DLManagedTensor {
    let ndim = shape.len();
    let mut strides = vec![1i64; ndim];
    if ndim >= 2 {
        for d in (0..ndim - 1).rev() {
            strides[d] = strides[d + 1] * shape[d + 1];
        }
    }

    let data_ptr = data.as_mut_ptr() as *mut c_void;
    let mut ctx = Box::new(OwnedDlPackContext {
        _data: data,
        _shape: shape,
        _strides: strides,
    });

    let shape_ptr = ctx._shape.as_mut_ptr();
    let strides_ptr = ctx._strides.as_mut_ptr();
    let ctx_ptr = Box::into_raw(ctx) as *mut c_void;

    let managed = Box::new(DLManagedTensor {
        dl_tensor: DLTensor {
            data: data_ptr,
            device: DLDevice {
                device_type: K_DL_CPU,
                device_id: 0,
            },
            ndim: ndim as i32,
            dtype: DLDataType {
                code: K_DL_FLOAT,
                bits: 64,
                lanes: 1,
            },
            shape: shape_ptr,
            strides: strides_ptr,
            byte_offset: 0,
        },
        manager_ctx: ctx_ptr,
        deleter: Some(dlpack_managed_deleter),
    });

    Box::into_raw(managed)
}

/// Python-visible Zero-Copy Tensor wrapper implementing `__dlpack__` and `__dlpack_device__`
/// for direct zero-copy import into `torch.from_dlpack(...)` and `mlx.core`.
#[pyclass]
#[derive(Clone, Debug)]
pub struct DlPackTensor {
    data: Vec<f64>,
    shape: Vec<i64>,
}

impl DlPackTensor {
    pub fn new_1d(data: Vec<f64>) -> Self {
        let len = data.len() as i64;
        Self {
            data,
            shape: vec![len],
        }
    }

    pub fn new_2d(data: Vec<f64>, rows: usize, cols: usize) -> Self {
        Self {
            data,
            shape: vec![rows as i64, cols as i64],
        }
    }

    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }
}

#[pymethods]
impl DlPackTensor {
    /// Returns `(device_type, device_id)` per the Python Array API / DLPack protocol.
    pub fn __dlpack_device__(&self) -> (i32, i32) {
        (K_DL_CPU, 0)
    }

    /// Exports a `PyCapsule` named `"dltensor"` holding a zero-copy `DLManagedTensor`.
    #[pyo3(signature = (stream=None))]
    pub fn __dlpack__<'py>(
        &self,
        py: Python<'py>,
        stream: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = stream;
        let ptr = into_dlpack_managed_tensor(self.data.clone(), self.shape.clone());
        let name = CString::new("dltensor").unwrap();
        unsafe {
            let cap_ptr = pyo3::ffi::PyCapsule_New(ptr as *mut c_void, name.as_ptr(), None);
            Bound::from_owned_ptr_or_err(py, cap_ptr)
        }
    }

    /// Returns a copy of the underlying data as a Python list.
    pub fn to_list(&self) -> Vec<f64> {
        self.data.clone()
    }

    /// Returns the tensor shape tuple.
    pub fn shape(&self) -> Vec<i64> {
        self.shape.clone()
    }
}

/// Python-accessible Differentiable Circuit Simulator (`rustspice_py.PySimulator`).
#[pyclass]
pub struct PySimulator {
    inner: Simulator,
}

#[pymethods]
impl PySimulator {
    #[new]
    pub fn new(spice_netlist: &str) -> PyResult<Self> {
        let sim = Simulator::from_spice(spice_netlist)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { inner: sim })
    }

    /// Sets a device parameter (`R`, `W`, `L`, `VTO`, `KP`, `IS`, `N`, `DC`) in-place.
    pub fn set_parameter(&mut self, device: &str, param: &str, value: f64) -> PyResult<()> {
        self.inner
            .topology
            .soa
            .set_device_parameter(device, param, value)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Solves the DC Operating Point and returns a zero-copy [`DlPackTensor`] of the MNA solution.
    pub fn solve_dc_dlpack(&mut self) -> PyResult<DlPackTensor> {
        let res = self
            .inner
            .run_dc_op()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(DlPackTensor::new_1d(res.solution))
    }

    /// Solves the DC Operating Point and returns `(node_voltage, Adjoint Gradients DlPackTensor)`
    /// for a list of `(device_name, param_name)` pairs in a single transposed LU solve!
    pub fn solve_dc_adjoint_dlpack(
        &mut self,
        target_node: &str,
        params: Vec<(String, String)>,
    ) -> PyResult<(f64, DlPackTensor)> {
        let specs: Vec<ParameterGradSpec> = params
            .into_iter()
            .map(|(d, p)| ParameterGradSpec::new(d, p))
            .collect();
        let (op, adj) = self
            .inner
            .run_dc_adjoint(target_node, &specs)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let v_out = self
            .inner
            .node_voltage(&op.solution, target_node)
            .unwrap_or(0.0);
        Ok((v_out, DlPackTensor::new_1d(adj.gradients)))
    }
}

/// Python module initializer (`import rustspice_py`).
#[pymodule]
pub fn rustspice_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<DlPackTensor>()?;
    m.add_class::<PySimulator>()?;
    let _ = ptr::null::<c_void>();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlpack_managed_tensor_zero_copy_layout_and_deleter() {
        let raw = vec![1.25_f64, 2.5_f64, 3.75_f64, 5.0_f64];
        let ptr = into_dlpack_managed_tensor(raw, vec![2, 2]);
        assert!(!ptr.is_null());

        unsafe {
            let tensor = &(*ptr).dl_tensor;
            assert_eq!(tensor.ndim, 2);
            assert_eq!(tensor.device.device_type, K_DL_CPU);
            assert_eq!(tensor.dtype.code, K_DL_FLOAT);
            assert_eq!(tensor.dtype.bits, 64);
            let shape_slice = std::slice::from_raw_parts(tensor.shape, 2);
            assert_eq!(shape_slice, &[2, 2]);
            let data_slice = std::slice::from_raw_parts(tensor.data as *const f64, 4);
            assert_eq!(data_slice, &[1.25, 2.5, 3.75, 5.0]);

            if let Some(deleter) = (*ptr).deleter {
                deleter(ptr);
            }
        }
    }
}
