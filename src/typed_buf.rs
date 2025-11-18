// src/typed_buffer.rs
use core::ffi::{c_char, c_long};
use crate::{raw, AtmiCtx, AtmiResult};

use std::{
    mem::ManuallyDrop,
};

#[derive(Debug)]
pub struct TypedBuffer<'ctx> {
    ptr: *mut c_char,    // may be null
    pub ctx: &'ctx AtmiCtx,  // real reference to the owning context
}

impl<'ctx> TypedBuffer<'ctx> {
    /// # Safety
    /// `raw` must be a valid `atmibuf*` allocated for this context and owned by the caller.
    pub unsafe fn from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        Self { ptr: raw, ctx }
    }

    /// Transfers ownership to C or another wrapper. No Drop is run.
    pub fn into_raw(self) -> *mut c_char {
        let me = ManuallyDrop::new(self);
        me.ptr
    }

    /// Return current C pointer.
    #[inline]
    pub fn as_ptr(&self) -> *mut c_char {
        self.ptr
    }

    /// # Safety
    /// Retie this buffer to a *different* context.
    ///
    /// Only valid if the underlying ATMI/UBF API actually allows this buffer
    /// to be used under `new_ctx`. The lifetime re-tie is unchecked by Rust.
    pub unsafe fn move_to_context<'new>(
        self,
        new_ctx: &'new AtmiCtx,
    ) -> TypedBuffer<'new> {
        let ptr = self.into_raw();
        // rewrap with new lifetime / context
        TypedBuffer::from_raw(new_ctx, ptr)
    }

    /// Reallocate this buffer with a new size using `tprealloc`.
    ///
    /// On success, `self` will point to the new buffer.
    /// On failure, `self` remains valid and unchanged, and the error is returned.
    pub fn tprealloc(&mut self, new_size: usize) -> AtmiResult<()> {
        let new_ptr = unsafe {
            raw::tprealloc(self.ptr as *mut c_char, new_size as c_long)
        };

        if new_ptr.is_null() {
            // C failed; original pointer still valid.
            // Use the error from *this* context instance.
            Err(self.ctx.atmi_last_error())
        } else {
            // Success, update pointer (may or may not have moved).
            self.ptr = new_ptr;
            Ok(())
        }
    }

}

impl<'ctx> Drop for TypedBuffer<'ctx> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { raw::tpfree(self.ptr) }
        }
    }
}
