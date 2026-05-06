use crate::{raw, AtmiError, AtmiResult, NstdError, TypedBuffer, TypedUbf, UbfError};
use core::ffi::{c_char, c_int, c_long};

#[cfg(feature = "ctx-send")]
use std::cell::Cell;
use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    ptr,
};

// --- Marker selection -------------------------------------------------------
#[cfg(not(feature = "ctx-send"))]
type CtxMarker = std::rc::Rc<()>; // -> !Send & !Sync

#[cfg(feature = "ctx-send")]
type CtxMarker = Cell<()>; // -> Send & !Sync

#[cfg(feature = "ctx-send")]
type CtxHandle = raw::TPCONTEXT_T;

/// Per-thread XATMI context.
///
/// By default the context is neither `Send` nor `Sync`. With the `ctx-send`
/// feature enabled it becomes `Send`, but remains `!Sync`.
#[derive(Debug)]
pub struct AtmiCtx {
    _marker: PhantomData<CtxMarker>,

    #[cfg(feature = "ctx-send")]
    handle: Cell<CtxHandle>,
}

impl AtmiCtx {
    /// Create a new ATMI context handle.
    pub fn new() -> Result<Self, AtmiError> {
        #[cfg(not(feature = "ctx-send"))]
        {
            Ok(AtmiCtx {
                _marker: PhantomData,
            })
        }

        #[cfg(feature = "ctx-send")]
        {
            unsafe {
                let handle = raw::tpnewctxt(0, 0);

                if handle.is_null() {
                    return Err(AtmiError::new(
                        raw::TPESYSTEM,
                        "failed to allocate ATMI context",
                    ));
                }

                Ok(AtmiCtx {
                    _marker: PhantomData,
                    handle: Cell::new(handle),
                })
            }
        }
    }

    /// Join the application as a client by calling `tpinit`.
    pub fn tpinit(&self) -> AtmiResult<()> {
        let rc = unsafe { raw::tpinit(ptr::null_mut()) };
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Leave the application by calling `tpterm`.
    pub fn tpterm(&self) -> AtmiResult<()> {
        let rc = unsafe { raw::tpterm() };
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Return last ATMI error for the current thread/context.
    pub fn atmi_last_error(&self) -> AtmiError {
        unsafe {
            // Adjust types to your actual FFI signatures.
            let err_ptr = raw::_exget_tperrno_addr(); // *const i32 or *mut i32
            let code = *err_ptr;
            let msg_ptr = raw::tpstrerror(code); // *const c_char
            let message = CStr::from_ptr(msg_ptr).to_string_lossy().into_owned();
            AtmiError::new(code as u32, message)
        }
    }

    /// Return last UBF error for the current thread/context.
    pub fn ubf_last_error(&self) -> UbfError {
        unsafe {
            let err_ptr = self.ndrx_bget_ferror_addr();
            let code = *err_ptr;
            let msg_ptr = self.bstrerror(code);
            let message = CStr::from_ptr(msg_ptr).to_string_lossy().into_owned();
            UbfError::new(code as u32, message)
        }
    }

    /// Return the last NSTD error for the current thread/context.
    pub fn nstd_last_error(&self) -> NstdError {
        unsafe {
            let err_ptr = raw::_Nget_Nerror_addr(); // *const i32 or *mut i32
            let code = *err_ptr;
            let msg_ptr = raw::Nstrerror(code); // *const c_char
            let message = CStr::from_ptr(msg_ptr).to_string_lossy().into_owned();
            NstdError::new(code as u32, message)
        }
    }

    /// Allocate a typed XATMI buffer tied to this context.
    pub fn tpalloc<'ctx>(
        &'ctx self,
        type_: &str,
        subtype: &str,
        size: usize,
    ) -> AtmiResult<TypedBuffer<'ctx>> {
        let type_c = CString::new(type_)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "type_ contains NUL byte"))?;
        let subtype_c = CString::new(subtype)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "subtype contains NUL byte"))?;

        let ptr = unsafe {
            raw::tpalloc(
                type_c.as_ptr() as *mut c_char,
                subtype_c.as_ptr() as *mut c_char,
                size as c_long,
            )
        };

        if ptr.is_null() {
            Err(self.atmi_last_error())
        } else {
            let buf = unsafe { TypedBuffer::from_raw(self, ptr) };
            Ok(buf)
        }
    }

    /// Allocate a CARRAY (binary array) buffer tied to this context, copy the
    /// provided bytes in, and set `len()` to `bytes.len()`.
    pub fn tpalloc_carray<'ctx>(&'ctx self, bytes: &[u8]) -> AtmiResult<TypedBuffer<'ctx>> {
        let size = bytes.len().max(1);
        let mut buf = self.tpalloc("CARRAY", "", size)?;
        if !bytes.is_empty() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    buf.as_ptr() as *mut u8,
                    bytes.len(),
                );
            }
        }
        buf.set_len(bytes.len());
        Ok(buf)
    }

    /// Allocate a UBF buffer tied to this context.
    pub fn tpalloc_ubf<'ctx>(&'ctx self, size: usize) -> AtmiResult<TypedUbf<'ctx>> {
        let type_c = CString::new("UBF").unwrap();
        let subtype_c = CString::new("").unwrap();

        let raw_ptr = unsafe {
            raw::tpalloc(
                type_c.as_ptr() as *mut c_char,
                subtype_c.as_ptr() as *mut c_char,
                size as c_long,
            )
        };

        if raw_ptr.is_null() {
            Err(self.atmi_last_error())
        } else {
            let ubf = unsafe { TypedUbf::from_raw(self, raw_ptr) };
            Ok(ubf)
        }
    }

    /*
    fn ubf_last_error() -> AtmiError { ... }
    fn nstd_last_error() -> AtmiError { ... }
    */

    #[cfg(feature = "ctx-send")]
    #[inline]
    pub(crate) fn c_ctx_ptr(&self) -> *mut raw::TPCONTEXT_T {
        self.handle.as_ptr()
    }
}

impl Drop for AtmiCtx {
    fn drop(&mut self) {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::tpterm();
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            let handle = self.handle.get();
            if !handle.is_null() {
                raw::tpfreectxt(handle);
                self.handle.set(ptr::null_mut());
            }
        }
    }
}
