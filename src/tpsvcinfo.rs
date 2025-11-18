use crate::{raw, AtmiCtx, TypedBuffer};
use core::ffi::c_char;
use std::{ffi::CStr};

/// Safe Rust wrapper for TPSVCINFO passed into a service callback.
///
/// This does **not** own the service buffer.
/// XATMI owns TPSVCINFO and its data; Rust only observes it.
#[derive(Debug)]
pub struct TpSvcInfo<'ctx> {
    raw: *mut raw::TPSVCINFO,
    ctx: &'ctx AtmiCtx,
}

///Service info returned by the service call
impl<'ctx> TpSvcInfo<'ctx> {
    /// # Safety
    /// - `raw` must be a valid TPSVCINFO pointer supplied by XATMI.
    /// - `ctx` must be the current ATMI context for this thread.
    /// - XATMI owns the memory; Rust must NOT free anything.
    pub unsafe fn from_raw(ctx: &'ctx AtmiCtx, raw: *mut raw::TPSVCINFO) -> Self {
        TpSvcInfo { raw, ctx }
    }

    #[inline]
    fn raw(&self) -> &raw::TPSVCINFO {
        unsafe { &*self.raw }
    }

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

    /// Name of the advertised function.
    pub fn fname(&self) -> &str {
        unsafe {
            CStr::from_ptr(self.raw().fname.as_ptr())
                .to_str()
                .unwrap_or("")
        }
    }

    /// Input buffer length.
    pub fn len(&self) -> i64 {
        self.raw().len
    }

    pub fn set_len(&mut self, len: i64) {
        self.raw_mut().len = len;
    }

    pub fn flags(&self) -> i64 {
        self.raw().flags
    }

    pub fn set_flags(&mut self, flags: i64) {
        self.raw_mut().flags = flags;
    }

    pub fn cd(&self) -> i32 {
        self.raw().cd
    }

    pub fn appkey(&self) -> i64 {
        self.raw().appkey
    }

    pub fn cltid(&self) -> raw::CLIENTID {
        self.raw().cltid
    }

    /// View the service buffer (`TPSVCINFO::data`) as a **TypedBufferView**.
    ///
    /// This buffer is **not owned** by Rust; XATMI controls its lifetime.
    ///
    /// # Safety
    /// You MUST ensure `ctx` matches the runtime's active context.
    pub fn data(&self) -> TypedBuffer<'ctx> {
        let ptr = self.raw().data as *mut c_char;
        let ret = unsafe { TypedBuffer::from_raw(self.ctx, ptr) };
        ret
    }
}

