use core::ffi::c_int;
use crate::{raw, AtmiError, AtmiResult};
use std::{ffi::CStr, marker::PhantomData, ptr};

// --- Marker selection -------------------------------------------------------
// Default: !Send & !Sync (thread-local only)
#[cfg(not(feature = "ctx-send"))]
type CtxMarker = std::rc::Rc<()>;      // -> !Send & !Sync

// With "ctx-send" feature: Send & !Sync (can be moved across threads, not shared)
#[cfg(feature = "ctx-send")]
type CtxMarker = std::cell::Cell<()>;  // -> Send & !Sync

/// Per-thread XATMI context.
/// - Default (no "ctx-send"): !Send & !Sync
/// - With "ctx-send": Send & !Sync
#[derive(Debug)]
pub struct AtmiCtx(PhantomData<CtxMarker>);

impl AtmiCtx {

    ///Estalish new ATMI context
    pub fn new() -> Self {
        Self(PhantomData)
    }

    /// Perform init (tpinit). On success current context becomes assocated with ATMI session.
    /// See *tpinit(3)* for more details.
    pub fn tpinit(&mut self) -> AtmiResult<()> {
        let rc = unsafe { raw::tpinit(ptr::null_mut()) };
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(Self::atmi_last_error())
        }
    }

    /// Perform un-init (tpterm). On success ATMI session is terminated
    /// See *tpinit(3)* for more details.
    pub fn tpterm(&mut self) -> AtmiResult<()> {
        let rc = unsafe { raw::tpterm() };
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(Self::atmi_last_error())
        }
    }

    /// Return last ATMI error for the current thread/context.
    fn atmi_last_error() -> AtmiError {
        unsafe {
            // Adjust types to your actual FFI signatures.
            let err_ptr = raw::_exget_tperrno_addr(); // *const i32 or *mut i32
            let code = *err_ptr;
            let msg_ptr = raw::tpstrerror(code);      // *const c_char
            let message = CStr::from_ptr(msg_ptr).to_string_lossy().into_owned();
            AtmiError::new(code as u16, message)
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
