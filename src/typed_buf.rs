use core::ffi::c_char;
use std::mem::ManuallyDrop;
use crate::{raw};

#[derive(Debug)]
pub struct TypedBuffer {
    ptr: *mut c_char,    // may be null
}

/// Typed buffer base structure
impl TypedBuffer {

    /// # Safety
    /// `raw` must be a valid `atmibuf*` allocated by the C side and owned by the caller.
    pub unsafe fn from_raw(raw: *mut c_char) -> Option<Self> {
        if raw.is_null() {
            None
        } else {
            Some(Self { ptr: raw })
        }
    }

    /// Transfers ownership to C (no Drop).
    pub fn into_raw(self) -> *mut c_char {
        let me = ManuallyDrop::new(self);
        me.ptr
    }

    /// Return current C pointer
    #[inline]
    pub fn as_ptr(&self) -> *mut c_char {
        self.ptr
    }

    //TODO: Add castings to other buffer types with validation of the buffer
}

//Free up the buffer
impl Drop for TypedBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { raw::tpfree(self.ptr); }
        }
    }
}

/*
#[derive(Debug, Clone, Copy)]
pub struct WrongKind {
    pub expected: AtmiKind,
    pub found: AtmiKind,
}
*/

