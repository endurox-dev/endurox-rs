//! Service request metadata and ownership of incoming typed buffers.
use crate::{raw, AtmiCtx, TypedBuffer, TypedUbf};
use core::ffi::c_char;
use std::ffi::CStr;

/// Safe Rust wrapper for TPSVCINFO passed into a service callback.
///
/// Owns the service buffer until the handler calls `take_data()` /
/// `take_data_ubf()` and passes it to `tpreturn`.  If the handler
/// returns without taking the data the buffer is freed automatically via
/// the `TypedBuffer` Drop impl (correct: service must either return or free it).
#[derive(Debug)]
pub struct TpSvcInfo<'ctx> {
    raw: *mut raw::TPSVCINFO,
    ctx: &'ctx AtmiCtx,
    /// Owned data buffer from TPSVCINFO::data.  Moved out on take_data*().
    data: Option<TypedBuffer<'ctx>>,
}

/// Request metadata access and transfer of the incoming service buffer.
impl<'ctx> TpSvcInfo<'ctx> {
    /// Wrap a native service request and take ownership of its incoming data buffer.
    ///
    /// # Safety
    /// - `raw` must be a valid TPSVCINFO pointer supplied by XATMI.
    /// - `ctx` must be the current ATMI context for this thread.
    pub(crate) unsafe fn from_raw(ctx: &'ctx AtmiCtx, raw: *mut raw::TPSVCINFO) -> Self {
        let data_ptr = (*raw).data as *mut c_char;
        let data = if data_ptr.is_null() {
            None
        } else {
            let len = (*raw).len.max(0) as usize;
            Some(TypedBuffer::from_raw_with_len(ctx, data_ptr, len))
        };
        TpSvcInfo { raw, ctx, data }
    }

    /// Borrow the native service metadata for reading.
    #[inline]
    fn raw(&self) -> &raw::TPSVCINFO {
        unsafe { &*self.raw }
    }

    /// Borrow the native service metadata for updating.
    #[inline]
    fn raw_mut(&mut self) -> &mut raw::TPSVCINFO {
        unsafe { &mut *self.raw }
    }

    /// Name of the service.
    pub fn name(&self) -> &str {
        unsafe {
            CStr::from_ptr(self.raw().name.as_ptr())
                .to_str()
                .unwrap_or("")
        }
    }

    /// Name of the advertised function (may differ from service name).
    pub fn fname(&self) -> &str {
        unsafe {
            CStr::from_ptr(self.raw().fname.as_ptr())
                .to_str()
                .unwrap_or("")
        }
    }

    /// Input buffer length as reported by the framework.
    pub fn len(&self) -> i64 {
        self.raw().len
    }

    /// Update the length in the service metadata; the owned buffer’s tracked length is unchanged.
    ///
    /// # Arguments
    ///
    /// - `len`: Request length in bytes to record in `TPSVCINFO`.
    pub fn set_len(&mut self, len: i64) {
        self.raw_mut().len = len;
    }

    /// Return the request’s native XATMI flags.
    pub fn flags(&self) -> i64 {
        self.raw().flags
    }

    /// Replace the flags in the service metadata.
    ///
    /// # Arguments
    ///
    /// - `flags`: Replacement XATMI request flag bitmask.
    pub fn set_flags(&mut self, flags: i64) {
        self.raw_mut().flags = flags;
    }

    /// Return the native call or conversation descriptor for this request.
    pub fn cd(&self) -> i32 {
        self.raw().cd
    }

    /// Return the requesting client’s application authentication key.
    pub fn appkey(&self) -> i64 {
        self.raw().appkey
    }

    /// Copy the client identifier used to send unsolicited notifications.
    pub fn cltid(&self) -> crate::ClientId {
        self.raw().cltid
    }

    /// Return the still-owned request buffer pointer, or null after it has been taken.
    pub(crate) fn data_ptr(&self) -> *mut c_char {
        self.data
            .as_ref()
            .map(|b| b.as_ptr())
            .unwrap_or(std::ptr::null_mut())
    }

    /// Transfer ownership of the service data buffer.
    ///
    /// Returns `None` if the buffer was already taken or was originally null.
    /// After calling this, the caller is responsible for passing the buffer
    /// to `tpreturn` (or freeing it before calling the raw API).
    pub fn take_data(&mut self) -> Option<TypedBuffer<'ctx>> {
        self.data.take()
    }

    /// Take the service data buffer as a typed UBF buffer.
    ///
    /// Convenience wrapper over `take_data()` for the common case where the
    /// request was sent as a UBF message.
    pub fn take_data_ubf(&mut self) -> Option<TypedUbf<'ctx>> {
        // A service that advertised a UBF interface but was handed something
        // else yields None rather than a UBF view over foreign bytes.
        self.data
            .take()
            .and_then(|buf| TypedUbf::from_typed(buf).ok())
    }
}
