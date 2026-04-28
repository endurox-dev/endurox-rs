use crate::{raw, AtmiCtx, AtmiResult};
use core::ffi::{c_char, c_long};

/// Owned XATMI typed buffer allocated by `tpalloc`.
///
/// The buffer is tied to the [`AtmiCtx`] that allocated it and is released with
/// `tpfree` when dropped.
#[derive(Debug)]
pub struct TypedBuffer<'ctx> {
    ptr: *mut c_char,
    pub(crate) ctx: &'ctx AtmiCtx,
    owned: bool,
}

impl<'ctx> TypedBuffer<'ctx> {
    /// # Safety
    /// `raw` must be a valid `atmibuf*` allocated for this context and owned by the caller.
    pub(crate) unsafe fn from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        Self {
            ptr: raw,
            ctx,
            owned: true,
        }
    }

    /// # Safety
    /// `raw` must be a valid `atmibuf*` owned by the caller for at least `'ctx`.
    pub(crate) unsafe fn borrowed_from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        Self {
            ptr: raw,
            ctx,
            owned: false,
        }
    }

    /// Transfer ownership of the underlying ATMI buffer pointer.
    ///
    /// The returned pointer will not be freed by this Rust value. Use this only
    /// when passing ownership to Enduro/X or immediately wrapping it in another
    /// owner.
    pub(crate) fn into_raw(self) -> *mut c_char {
        let ptr = self.ptr;
        std::mem::forget(self);
        ptr
    }

    /// Return the current ATMI buffer pointer without transferring ownership.
    ///
    /// This is intended for low-level integration with APIs that are not yet
    /// represented by a safe Rust wrapper.
    #[inline]
    pub(crate) fn as_ptr(&self) -> *mut c_char {
        self.ptr
    }

    /// # Safety
    /// Retie this buffer to a *different* context.
    ///
    /// Only valid if the underlying ATMI/UBF API actually allows this buffer
    /// to be used under `new_ctx`. The lifetime re-tie is unchecked by Rust.
    pub(crate) unsafe fn move_to_context<'new>(self, new_ctx: &'new AtmiCtx) -> TypedBuffer<'new> {
        TypedBuffer::from_raw(new_ctx, self.into_raw())
    }

    /// Update the internal pointer after a C API may have reallocated the buffer.
    #[inline]
    pub(crate) fn replace_ptr(&mut self, new_ptr: *mut c_char) {
        self.ptr = new_ptr;
    }

    /// Reallocate this buffer with a new size using `tprealloc`.
    ///
    /// On success, `self` will point to the new buffer.
    /// On failure, `self` remains valid and unchanged, and the error is returned.
    pub fn tprealloc(&mut self, new_size: usize) -> AtmiResult<()> {
        let new_ptr = unsafe { raw::tprealloc(self.ptr as *mut c_char, new_size as c_long) };

        if new_ptr.is_null() {
            Err(self.ctx.atmi_last_error())
        } else {
            self.ptr = new_ptr;
            Ok(())
        }
    }
}

impl<'ctx> Drop for TypedBuffer<'ctx> {
    fn drop(&mut self) {
        if self.owned && !self.ptr.is_null() {
            unsafe { raw::tpfree(self.ptr) }
        }
    }
}
