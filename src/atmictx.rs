use core::ffi::{c_int, c_long, c_char};
use crate::{raw, AtmiError, AtmiResult, TypedBuffer, TypedUbf};

use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    ptr,
};


// --- Marker selection -------------------------------------------------------
#[cfg(not(feature = "ctx-send"))]
type CtxMarker = std::rc::Rc<()>; // -> !Send & !Sync

#[cfg(feature = "ctx-send")]
type CtxMarker = std::cell::Cell<()>; // -> Send & !Sync

#[cfg(feature = "ctx-send")]
type CtxHandle = raw::TPCONTEXT_T;

/// Per-thread XATMI context.
/// - Default (no "ctx-send"): !Send & !Sync
/// - With "ctx-send": Send & !Sync
#[derive(Debug)]
pub struct AtmiCtx {

    _marker: PhantomData<CtxMarker>,

    #[cfg(feature = "ctx-send")]
    handle: CtxHandle,
}

impl AtmiCtx {

    ///Estalish new ATMI context
    pub fn new() -> Result<Self, AtmiError> {

        // No ctx-send: just a thread-local marker, nothing to allocate.
        #[cfg(not(feature = "ctx-send"))]
        {
            Ok(AtmiCtx { _marker: PhantomData })
        }

        // With ctx-send: allocate context on C side via tpnewctxt(0, 0).
        #[cfg(feature = "ctx-send")]
        {
            unsafe {
                // Adjust signature to your actual raw binding:
                // e.g. fn tpnewctxt(flags: i64, rsvd: i64) -> raw::TPCONTEXT_T;
                let handle = raw::tpnewctxt(0, 0);

                // If TPCONTEXT_T is a pointer type, check for null:
                // if handle.is_null() { ... }

                if handle_invalid(handle) {
                    // replace `handle_invalid` with your real check if needed
                    return Err(AtmiError::new(
                        raw::TPESYSTEM,
                        "Failed to allocate new context - see ULOG for details",
                    ));
                }

                Ok(AtmiCtx {
                    _marker: PhantomData,
                    handle,
                })
            }
        }
    }

    /// Perform init (tpinit). On success current context becomes assocated with ATMI session.
    /// See *tpinit(3)* for more details.
    pub fn tpinit(&self) -> AtmiResult<()> {
        let rc = unsafe { raw::tpinit(ptr::null_mut()) };
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Perform un-init (tpterm). On success ATMI session is terminated
    /// See *tpinit(3)* for more details.
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
            let msg_ptr = raw::tpstrerror(code);      // *const c_char
            let message = CStr::from_ptr(msg_ptr).to_string_lossy().into_owned();
            AtmiError::new(code as u32, message)
        }
    }

    /// Generic tpalloc -> lifetime-tied `TypedBuffer<'ctx>`.
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
            // SAFETY: just allocated, owned by this context.
            let buf = unsafe { TypedBuffer::from_raw(self, ptr) }
                .expect("tpalloc returned non-null pointer but from_raw returned None");
            Ok(buf)
        }
    }

    /// Typed helper: UBF buffer.
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
            // SAFETY: just allocated UBF buffer for this context.
            let ubf = unsafe { TypedUbf::from_raw(self, raw_ptr) }
                .expect("tpalloc_ubf returned non-null pointer but from_raw returned None");
            Ok(ubf)
        }
    }

    /*
    fn ubf_last_error() -> AtmiError { ... }
    fn nstd_last_error() -> AtmiError { ... }
    */
}

impl Drop for AtmiCtx {
    fn drop(&mut self) {
        unsafe { raw::tpterm(); }
    }
}
