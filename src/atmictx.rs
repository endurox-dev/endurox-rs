use crate::raw;
use std::{marker::PhantomData, ptr};

/// Per-thread XATMI context. Not Send/Sync (Enduro/X client state is thread-local).
#[derive(Debug)]
pub struct AtmiCtx(PhantomData<*mut ()>);

// ATMI Context
impl AtmiCtx {
    pub fn init() -> Result<Self, i32> {
        let rc = unsafe { raw::tpinit(ptr::null_mut()) };
        if rc == 0 {
            Ok(Self(PhantomData))
        } else {
            // best effort: read thread-local tperrno if available
            let code = unsafe { *raw::_exget_tperrno_addr() };
            Err(code)
        }
    }
}

impl Drop for AtmiCtx {
    fn drop(&mut self) {
        unsafe { raw::tpterm(); }
    }
}

