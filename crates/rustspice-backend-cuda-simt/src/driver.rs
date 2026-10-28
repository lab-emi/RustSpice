//! Minimal CUDA driver API, loaded at run time from `libcuda` (the NVIDIA driver), so that RustSpice builds and runs
//! on machines without CUDA: [`CudaDriver::load`] fails and the callers stay on the CPU.
//!
//! Only what the batched kernels need: the primary context of one device, PTX modules (JIT-compiled by the driver
//! for the installed GPU), linear device memory, synchronous copies and launches.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::sync::Arc;

use libloading::Library;

type CuResult = c_int;
type CuDevice = c_int;
type CuContext = *mut c_void;
type CuModule = *mut c_void;
type CuFunction = *mut c_void;
type CuDevicePtr = u64;

const CUDA_SUCCESS: CuResult = 0;

struct Api {
    init: unsafe extern "C" fn(c_uint) -> CuResult,
    driver_get_version: unsafe extern "C" fn(*mut c_int) -> CuResult,
    device_get_count: unsafe extern "C" fn(*mut c_int) -> CuResult,
    device_get: unsafe extern "C" fn(*mut CuDevice, c_int) -> CuResult,
    device_get_name: unsafe extern "C" fn(*mut c_char, c_int, CuDevice) -> CuResult,
    primary_ctx_retain: unsafe extern "C" fn(*mut CuContext, CuDevice) -> CuResult,
    primary_ctx_release: unsafe extern "C" fn(CuDevice) -> CuResult,
    ctx_set_current: unsafe extern "C" fn(CuContext) -> CuResult,
    ctx_synchronize: unsafe extern "C" fn() -> CuResult,
    module_load_data_ex: unsafe extern "C" fn(
        *mut CuModule,
        *const c_void,
        c_uint,
        *mut c_int,
        *mut *mut c_void,
    ) -> CuResult,
    module_unload: unsafe extern "C" fn(CuModule) -> CuResult,
    module_get_function: unsafe extern "C" fn(*mut CuFunction, CuModule, *const c_char) -> CuResult,
    mem_alloc: unsafe extern "C" fn(*mut CuDevicePtr, usize) -> CuResult,
    mem_free: unsafe extern "C" fn(CuDevicePtr) -> CuResult,
    memcpy_htod: unsafe extern "C" fn(CuDevicePtr, *const c_void, usize) -> CuResult,
    memcpy_dtoh: unsafe extern "C" fn(*mut c_void, CuDevicePtr, usize) -> CuResult,
    #[allow(clippy::type_complexity)]
    launch_kernel: unsafe extern "C" fn(
        CuFunction,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        *mut c_void,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> CuResult,
    get_error_name: unsafe extern "C" fn(CuResult, *mut *const c_char) -> CuResult,
    mem_get_info: unsafe extern "C" fn(*mut usize, *mut usize) -> CuResult,
    // keeps the function pointers above valid
    _lib: Library,
}

/// Error of a driver call: the API function and the driver's error name.
#[derive(Clone, Debug)]
pub struct CudaError(pub String);

impl std::fmt::Display for CudaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CUDA driver: {}", self.0)
    }
}

impl std::error::Error for CudaError {}

/// The primary context of one CUDA device.
pub struct CudaDriver {
    api: Api,
    device: CuDevice,
    ctx: CuContext,
    /// Device name as the driver reports it.
    pub device_name: String,
    /// CUDA version of the driver (`1000 * major + 10 * minor`).
    pub driver_version: i32,
}

// The driver API is thread safe; every call below first binds the context to the calling thread.
unsafe impl Send for CudaDriver {}
unsafe impl Sync for CudaDriver {}

macro_rules! sym {
    ($lib:expr, $name:literal) => {
        // SAFETY: the signature is the documented driver API prototype of `$name`.
        *unsafe { $lib.get($name) }.map_err(|e| {
            CudaError(format!(
                "{} not found: {e}",
                String::from_utf8_lossy(&$name[..$name.len() - 1])
            ))
        })?
    };
}

impl CudaDriver {
    /// Loads the driver and retains the primary context of device `ordinal`.
    pub fn load(ordinal: usize) -> Result<Arc<Self>, CudaError> {
        let lib = ["libcuda.so.1", "libcuda.so", "nvcuda.dll"]
            .iter()
            // SAFETY: loading the NVIDIA driver library runs no initialisation code beyond the library's own.
            .find_map(|name| unsafe { Library::new(name) }.ok())
            .ok_or_else(|| CudaError("libcuda not found (no NVIDIA driver)".to_string()))?;
        let api = Api {
            init: sym!(lib, b"cuInit\0"),
            driver_get_version: sym!(lib, b"cuDriverGetVersion\0"),
            device_get_count: sym!(lib, b"cuDeviceGetCount\0"),
            device_get: sym!(lib, b"cuDeviceGet\0"),
            device_get_name: sym!(lib, b"cuDeviceGetName\0"),
            primary_ctx_retain: sym!(lib, b"cuDevicePrimaryCtxRetain\0"),
            primary_ctx_release: sym!(lib, b"cuDevicePrimaryCtxRelease_v2\0"),
            ctx_set_current: sym!(lib, b"cuCtxSetCurrent\0"),
            ctx_synchronize: sym!(lib, b"cuCtxSynchronize\0"),
            module_load_data_ex: sym!(lib, b"cuModuleLoadDataEx\0"),
            module_unload: sym!(lib, b"cuModuleUnload\0"),
            module_get_function: sym!(lib, b"cuModuleGetFunction\0"),
            mem_alloc: sym!(lib, b"cuMemAlloc_v2\0"),
            mem_free: sym!(lib, b"cuMemFree_v2\0"),
            memcpy_htod: sym!(lib, b"cuMemcpyHtoD_v2\0"),
            memcpy_dtoh: sym!(lib, b"cuMemcpyDtoH_v2\0"),
            launch_kernel: sym!(lib, b"cuLaunchKernel\0"),
            get_error_name: sym!(lib, b"cuGetErrorName\0"),
            mem_get_info: sym!(lib, b"cuMemGetInfo_v2\0"),
            _lib: lib,
        };
        let check = |what: &str, r: CuResult| Self::check_with(&api, what, r);
        unsafe {
            check("cuInit", (api.init)(0))?;
            let mut driver_version = 0;
            check(
                "cuDriverGetVersion",
                (api.driver_get_version)(&mut driver_version),
            )?;
            let mut count = 0;
            check("cuDeviceGetCount", (api.device_get_count)(&mut count))?;
            if ordinal >= count as usize {
                return Err(CudaError(format!(
                    "device {ordinal} requested, {count} present"
                )));
            }
            let mut device = 0;
            check(
                "cuDeviceGet",
                (api.device_get)(&mut device, ordinal as c_int),
            )?;
            let mut name = [0 as c_char; 256];
            check(
                "cuDeviceGetName",
                (api.device_get_name)(name.as_mut_ptr(), name.len() as c_int, device),
            )?;
            let device_name = CStr::from_ptr(name.as_ptr()).to_string_lossy().into_owned();
            let mut ctx = std::ptr::null_mut();
            check(
                "cuDevicePrimaryCtxRetain",
                (api.primary_ctx_retain)(&mut ctx, device),
            )?;
            Ok(Arc::new(Self {
                api,
                device,
                ctx,
                device_name,
                driver_version,
            }))
        }
    }

    fn check_with(api: &Api, what: &str, r: CuResult) -> Result<(), CudaError> {
        if r == CUDA_SUCCESS {
            return Ok(());
        }
        let mut name: *const c_char = std::ptr::null();
        // SAFETY: `cuGetErrorName` stores a pointer to a static string (or leaves it null).
        let text = unsafe {
            if (api.get_error_name)(r, &mut name) == CUDA_SUCCESS && !name.is_null() {
                CStr::from_ptr(name).to_string_lossy().into_owned()
            } else {
                format!("error {r}")
            }
        };
        Err(CudaError(format!("{what}: {text}")))
    }

    fn check(&self, what: &str, r: CuResult) -> Result<(), CudaError> {
        Self::check_with(&self.api, what, r)
    }

    fn bind(&self) -> Result<(), CudaError> {
        // SAFETY: `ctx` is the retained primary context.
        self.check("cuCtxSetCurrent", unsafe {
            (self.api.ctx_set_current)(self.ctx)
        })
    }

    /// Free and total device memory in bytes.
    pub fn mem_info(&self) -> Result<(usize, usize), CudaError> {
        self.bind()?;
        let (mut free, mut total) = (0usize, 0usize);
        // SAFETY: two out-pointers to locals.
        self.check("cuMemGetInfo", unsafe {
            (self.api.mem_get_info)(&mut free, &mut total)
        })?;
        Ok((free, total))
    }

    /// Loads a PTX module (JIT-compiled by the driver for this device), with at most `max_registers` registers per
    /// thread when given (the rest spills to local memory, which buys occupancy).
    pub fn load_ptx(
        self: &Arc<Self>,
        ptx: &str,
        max_registers: Option<u32>,
    ) -> Result<CudaModule, CudaError> {
        self.bind()?;
        let text = CString::new(ptx).map_err(|_| CudaError("PTX contains a NUL byte".into()))?;
        let mut module = std::ptr::null_mut();
        // `CU_JIT_MAX_REGISTERS` (0); an unsigned option value travels in the pointer itself
        let mut opts: Vec<c_int> = Vec::new();
        let mut vals: Vec<*mut c_void> = Vec::new();
        if let Some(r) = max_registers {
            opts.push(0);
            vals.push(r as usize as *mut c_void);
        }
        // SAFETY: `text` is a NUL-terminated PTX image; `opts` / `vals` hold `opts.len()` entries; all outlive the
        // call.
        self.check("cuModuleLoadDataEx", unsafe {
            (self.api.module_load_data_ex)(
                &mut module,
                text.as_ptr().cast(),
                opts.len() as c_uint,
                opts.as_mut_ptr(),
                vals.as_mut_ptr(),
            )
        })?;
        Ok(CudaModule {
            driver: Arc::clone(self),
            module,
        })
    }

    /// Allocates `len` uninitialised elements of `T` in device memory.
    pub fn alloc<T: Copy>(self: &Arc<Self>, len: usize) -> Result<DeviceBuffer<T>, CudaError> {
        self.bind()?;
        let bytes = len.max(1) * std::mem::size_of::<T>();
        let mut ptr = 0;
        // SAFETY: out-pointer to a local.
        self.check("cuMemAlloc", unsafe {
            (self.api.mem_alloc)(&mut ptr, bytes)
        })?;
        Ok(DeviceBuffer {
            driver: Arc::clone(self),
            ptr,
            len,
            _t: std::marker::PhantomData,
        })
    }

    /// Copies `host` into a new device buffer.
    pub fn upload<T: Copy>(self: &Arc<Self>, host: &[T]) -> Result<DeviceBuffer<T>, CudaError> {
        let buf = self.alloc::<T>(host.len())?;
        buf.copy_from(host)?;
        Ok(buf)
    }

    /// Waits for every launch of this context.
    pub fn synchronize(&self) -> Result<(), CudaError> {
        self.bind()?;
        // SAFETY: no arguments.
        self.check("cuCtxSynchronize", unsafe { (self.api.ctx_synchronize)() })
    }
}

impl Drop for CudaDriver {
    fn drop(&mut self) {
        // SAFETY: releases the retain of `load`.
        unsafe {
            (self.api.primary_ctx_release)(self.device);
        }
    }
}

/// Device memory holding `len` elements of `T`.
pub struct DeviceBuffer<T: Copy> {
    driver: Arc<CudaDriver>,
    ptr: CuDevicePtr,
    len: usize,
    _t: std::marker::PhantomData<T>,
}

impl<T: Copy> DeviceBuffer<T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Device address.
    pub fn device_ptr(&self) -> u64 {
        self.ptr
    }

    /// Copies `host` (at most `len` elements) to the start of the buffer.
    pub fn copy_from(&self, host: &[T]) -> Result<(), CudaError> {
        assert!(host.len() <= self.len);
        if host.is_empty() {
            return Ok(());
        }
        self.driver.bind()?;
        // SAFETY: `host` is readable for its length, the buffer holds at least that many elements.
        self.driver.check("cuMemcpyHtoD", unsafe {
            (self.driver.api.memcpy_htod)(
                self.ptr,
                host.as_ptr().cast(),
                std::mem::size_of_val(host),
            )
        })
    }

    /// Copies the first `host.len()` elements of the buffer into `host`.
    pub fn copy_to(&self, host: &mut [T]) -> Result<(), CudaError> {
        assert!(host.len() <= self.len);
        if host.is_empty() {
            return Ok(());
        }
        self.driver.bind()?;
        // SAFETY: `host` is writable for its length, the buffer holds at least that many elements.
        self.driver.check("cuMemcpyDtoH", unsafe {
            (self.driver.api.memcpy_dtoh)(
                host.as_mut_ptr().cast(),
                self.ptr,
                std::mem::size_of_val(host),
            )
        })
    }
}

impl<T: Copy> Drop for DeviceBuffer<T> {
    fn drop(&mut self) {
        if self.driver.bind().is_ok() {
            // SAFETY: `ptr` came from `cuMemAlloc` and is freed once.
            unsafe {
                (self.driver.api.mem_free)(self.ptr);
            }
        }
    }
}

/// A loaded PTX module.
pub struct CudaModule {
    driver: Arc<CudaDriver>,
    module: CuModule,
}

/// One kernel argument, passed by value.
#[derive(Clone, Copy, Debug)]
pub enum KernelArg {
    U32(u32),
    U64(u64),
}

impl CudaModule {
    /// Launches `name` on a 1-D grid of `blocks` x `threads`, with `shared_bytes` of dynamic shared memory per block,
    /// and waits for it.
    ///
    /// # Safety
    /// `args` must match the kernel's parameter list, and every device pointer among them must cover what the
    /// kernel accesses for this grid.
    pub unsafe fn launch(
        &self,
        name: &str,
        blocks: u32,
        threads: u32,
        shared_bytes: u32,
        args: &[KernelArg],
    ) -> Result<(), CudaError> {
        let d = &self.driver;
        d.bind()?;
        let cname = CString::new(name).map_err(|_| CudaError("kernel name".into()))?;
        let mut func = std::ptr::null_mut();
        // SAFETY: out-pointer to a local; `cname` is NUL terminated.
        d.check("cuModuleGetFunction", unsafe {
            (d.api.module_get_function)(&mut func, self.module, cname.as_ptr())
        })?;
        let mut values: Vec<u64> = args
            .iter()
            .map(|a| match *a {
                KernelArg::U32(v) => u64::from(v),
                KernelArg::U64(v) => v,
            })
            .collect();
        // each parameter is read from its own slot: a `u32` from the low (first, little endian) four bytes
        let mut ptrs: Vec<*mut c_void> = values
            .iter_mut()
            .map(|v| (v as *mut u64).cast::<c_void>())
            .collect();
        // SAFETY: the caller vouches for the argument list; `ptrs` and `values` outlive the call.
        d.check("cuLaunchKernel", unsafe {
            (d.api.launch_kernel)(
                func,
                blocks,
                1,
                1,
                threads,
                1,
                1,
                shared_bytes,
                std::ptr::null_mut(),
                ptrs.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        })?;
        d.synchronize()
    }
}

impl Drop for CudaModule {
    fn drop(&mut self) {
        if self.driver.bind().is_ok() {
            // SAFETY: `module` came from `cuModuleLoadData` and is unloaded once.
            unsafe {
                (self.driver.api.module_unload)(self.module);
            }
        }
    }
}
