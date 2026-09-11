//! Native typed-buffer ownership, tracked payload lengths, and checked byte access.
use crate::{raw, AtmiCtx, AtmiError, AtmiResult};
use core::ffi::{c_char, c_long};
use std::ffi::CStr;

/// Result of [`TypedBuffer::tptypes`].
///
/// Mirrors the C `tptypes(3)` outputs: the buffer's reported allocation size,
/// the type name (e.g. `UBF`, `CARRAY`, `STRING`, `JSON`, `VIEW`), and the
/// subtype (e.g. the VIEW name). For types that have no subtype the
/// `subtype` field is an empty string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpTypeInfo {
    /// Buffer allocation size in bytes, as reported by `tptypes(3)`.
    pub size: usize,
    /// Buffer type name (e.g. `"UBF"`, `"CARRAY"`, `"STRING"`).
    pub type_name: String,
    /// Buffer subtype (e.g. VIEW name); empty when the buffer has no subtype.
    pub subtype: String,
}

/// Owned XATMI typed buffer allocated by `tpalloc`.
///
/// The buffer is tied to the [`AtmiCtx`] that allocated it and is released with
/// `tpfree` when dropped.
///
/// `len` carries the user data length used as `ilen`/`olen` for XATMI calls
/// (`tpcall`, `tpacall`, `tpsend`, `tpreturn`, ...). It is meaningful for
/// length-tracked buffer types (CARRAY, STRING) and may be left at `0` for
/// self-describing types (UBF, VIEW, JSON), where Enduro/X derives the length
/// from the buffer header.
#[derive(Debug)]
pub struct TypedBuffer<'ctx> {
    ptr: *mut c_char,
    pub(crate) ctx: &'ctx AtmiCtx,
    owned: bool,
    len: usize,
}

/// Typed allocation ownership, payload-length tracking, and checked buffer access.
impl<'ctx> TypedBuffer<'ctx> {
    /// Take ownership of a native typed buffer with an initially untracked payload length.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context that remains borrowed while this buffer wrapper exists.
    /// - `raw`: Native typed-buffer allocation to wrap.
    ///
    /// # Safety
    /// `raw` must be a valid `atmibuf*` allocated for this context and owned by the caller.
    pub(crate) unsafe fn from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        Self {
            ptr: raw,
            ctx,
            owned: true,
            len: 0,
        }
    }

    /// Take ownership of a native typed buffer and record its payload length.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context that remains borrowed while this buffer wrapper exists.
    /// - `raw`: Native typed-buffer allocation to wrap.
    /// - `len`: Logical payload length in bytes, separate from allocation capacity.
    ///
    /// # Safety
    /// `raw` must be a valid `atmibuf*` allocated for this context and owned by the caller.
    pub(crate) unsafe fn from_raw_with_len(
        ctx: &'ctx AtmiCtx,
        raw: *mut c_char,
        len: usize,
    ) -> Self {
        Self {
            ptr: raw,
            ctx,
            owned: true,
            len,
        }
    }

    /// Wrap a native typed buffer without taking responsibility for freeing it.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context that remains borrowed while this buffer wrapper exists.
    /// - `raw`: Native typed-buffer allocation to wrap.
    ///
    /// # Safety
    /// `raw` must be a valid `atmibuf*` owned by the caller for at least `'ctx`.
    pub(crate) unsafe fn borrowed_from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        Self {
            ptr: raw,
            ctx,
            owned: false,
            len: 0,
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

    /// Transfer this buffer to a wrapper borrowing another ATMI context.
    ///
    /// # Arguments
    ///
    /// - `new_ctx`: Context to borrow for the returned buffer’s lifetime.
    ///
    /// # Safety
    /// Retie this buffer to a *different* context.
    ///
    /// Only valid if the underlying ATMI/UBF API actually allows this buffer
    /// to be used under `new_ctx`. The lifetime re-tie is unchecked by Rust.
    pub(crate) unsafe fn move_to_context<'new>(self, new_ctx: &'new AtmiCtx) -> TypedBuffer<'new> {
        let len = self.len;
        TypedBuffer::from_raw_with_len(new_ctx, self.into_raw(), len)
    }

    /// Update the internal pointer after a C API may have reallocated the buffer.
    ///
    /// # Arguments
    ///
    /// - `new_ptr`: Pointer returned by the native operation; the old pointer is not freed here.
    ///
    #[inline]
    pub(crate) fn replace_ptr(&mut self, new_ptr: *mut c_char) {
        self.ptr = new_ptr;
    }

    /// Current user data length in bytes used as `ilen`/`olen` for XATMI calls.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if [`Self::len`] is zero.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Record a length Enduro/X itself reported for this buffer.
    ///
    /// # Arguments
    ///
    /// - `len`: Logical payload length in bytes, separate from allocation capacity.
    ///
    /// Internal, and unchecked by design: the value comes from an `olen`
    /// out-parameter, so it is within the allocation by construction. Doing a
    /// `tptypes()` round trip to re-verify it would add an FFI call to every
    /// call path.
    #[inline]
    pub(crate) fn set_len_reported(&mut self, len: usize) {
        self.len = len;
    }

    /// Allocation size in bytes, as Enduro/X reports it.
    fn capacity(&self) -> usize {
        self.tptypes().map(|info| info.size).unwrap_or(0)
    }

    /// Reject byte-level operations on buffers whose layout Enduro/X owns.
    ///
    /// # Arguments
    ///
    /// - `op`: Operation name included in a wrong-buffer-type error.
    ///
    /// CARRAY is the only type whose payload is a plain byte range carrying a
    /// separately tracked length. UBF and VIEW hold an internal header that raw
    /// writes corrupt, and STRING/JSON are NUL-terminated, so writing across the
    /// whole allocation drops the terminator. Each of those has its own typed
    /// accessors ([`crate::TypedUbf`], [`crate::TypedView`], `bwrite`/`bread`).
    ///
    /// Returns the type info so callers get the allocation size from the same
    /// lookup. `tptypes` takes a process-global mutex and walks the buffer hash
    /// (`ndrx_find_buffer`, `libatmi/typed_buf.c`), which measures around 130ns
    /// here against 12ns for a `Bfldtype` bit shift, so the type check and the
    /// bound must not cost two separate round trips.
    fn require_carray(&self, op: &str) -> AtmiResult<TpTypeInfo> {
        let info = self.tptypes()?;
        if info.type_name != "CARRAY" {
            return Err(AtmiError::new(
                raw::TPEINVAL,
                format!(
                    "{op} is only valid on a CARRAY buffer, this one is {}",
                    info.type_name
                ),
            ));
        }
        Ok(info)
    }

    /// Set the user data length in bytes used as `ilen` for XATMI calls.
    ///
    /// # Arguments
    ///
    /// - `len`: Logical payload length in bytes, separate from allocation capacity.
    ///
    /// CARRAY only. UBF and VIEW use native layouts and must be modified through
    /// their typed accessors. This method changes only the tracked length.
    ///
    /// Fails with `TPEINVAL` if `len` exceeds the allocation. Accepting a larger
    /// value would make [`Self::as_bytes`] construct a slice past the end of the
    /// buffer, which is undefined behaviour reachable from safe code.
    pub fn set_len(&mut self, len: usize) -> AtmiResult<()> {
        let cap = self.require_carray("set_len")?.size;
        if len > cap {
            return Err(AtmiError::new(
                raw::TPEINVAL,
                format!("length {len} exceeds the {cap} byte buffer allocation"),
            ));
        }

        // No zeroing here. CARRAY allocations are cleared when they are made
        // and when they grow (see `AtmiCtx::tpalloc` and `Self::tprealloc`), so
        // every byte within the allocation is already initialised. Writing here
        // instead would destroy live data: the length of a buffer received from
        // Enduro/X, or recovered from a BFLD_PTR field, is not known to this
        // wrapper, so a caller restating it would have had the payload
        // overwritten with nulls.
        self.len = len;
        Ok(())
    }

    /// View the user-data portion of the buffer as bytes (length tracked by `len()`).
    ///
    /// Clamped to the current allocation. Enduro/X can shrink a buffer behind a
    /// recorded length, so the tracked value alone is not a safe slice bound.
    pub fn as_bytes(&self) -> &[u8] {
        let len = self.len.min(self.capacity());
        if self.ptr.is_null() || len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr as *const u8, len) }
        }
    }

    /// Mutable byte view of the user-data portion of the buffer.
    ///
    /// CARRAY only, and clamped to the current allocation as [`Self::as_bytes`]
    /// is. Handing out a `&mut [u8]` over a UBF lets safe code rewrite the
    /// header or plant an arbitrary address in a `BFLD_PTR` occurrence, which
    /// the parent then frees.
    pub fn as_bytes_mut(&mut self) -> AtmiResult<&mut [u8]> {
        let len = self.len.min(self.require_carray("as_bytes_mut")?.size);
        if self.ptr.is_null() || len == 0 {
            Ok(&mut [])
        } else {
            Ok(unsafe { std::slice::from_raw_parts_mut(self.ptr as *mut u8, len) })
        }
    }

    /// Copy `bytes` into the buffer, growing it via [`Self::tprealloc`] if needed,
    /// and update `len()` to `bytes.len()`.
    ///
    /// # Arguments
    ///
    /// - `bytes`: Replacement CARRAY payload; its length becomes the tracked payload length.
    ///
    /// CARRAY only, for the reasons given on [`Self::as_bytes_mut`]. Copying a
    /// serialised UBF in with this would reproduce its pointer fields as bare
    /// addresses, leaving the parent to free buffers it never owned.
    pub fn set_bytes(&mut self, bytes: &[u8]) -> AtmiResult<()> {
        let info = self.require_carray("set_bytes")?;
        if bytes.len() > info.size {
            self.tprealloc(bytes.len())?;
        }
        if !bytes.is_empty() {
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr as *mut u8, bytes.len());
            }
        }
        self.len = bytes.len();
        Ok(())
    }

    /// Query the buffer via `tptypes(3)`: returns size plus type and subtype.
    pub fn tptypes(&self) -> AtmiResult<TpTypeInfo> {
        let mut type_buf = [0i8; raw::XATMI_TYPE_LEN as usize];
        let mut subtype_buf = [0i8; raw::XATMI_SUBTYPE_LEN as usize];

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tptypes(
                self.ptr,
                type_buf.as_mut_ptr() as *mut c_char,
                subtype_buf.as_mut_ptr() as *mut c_char,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otptypes(
                self.ctx.c_ctx_ptr(),
                self.ptr,
                type_buf.as_mut_ptr() as *mut c_char,
                subtype_buf.as_mut_ptr() as *mut c_char,
            )
        };

        if rc < 0 {
            Err(self.ctx.atmi_last_error())
        } else {
            let type_name = unsafe { CStr::from_ptr(type_buf.as_ptr() as *const c_char) }
                .to_string_lossy()
                .into_owned();
            let subtype = unsafe { CStr::from_ptr(subtype_buf.as_ptr() as *const c_char) }
                .to_string_lossy()
                .into_owned();
            Ok(TpTypeInfo {
                size: rc as usize,
                type_name,
                subtype,
            })
        }
    }

    /// Reallocate this buffer with a new size using `tprealloc`.
    ///
    /// # Arguments
    ///
    /// - `new_size`: Requested allocation size in bytes; must fit the native length type and
    ///   any VIEW layout.
    ///
    /// On success, `self` will point to the new buffer.
    /// On failure, `self` remains valid and unchanged, and the error is returned.
    pub fn tprealloc(&mut self, new_size: usize) -> AtmiResult<()> {
        // A VIEW is read through a fixed C struct layout, so the accessors keep
        // using offsets from the compiled view no matter how small the
        // allocation becomes. Shrinking below `Bvsizeof` turns every subsequent
        // field read into an out-of-bounds access from safe code.
        // One lookup serves both the view guard below and the CARRAY clearing
        // afterwards; `tptypes` takes a process-global mutex, so it is not
        // worth doing twice.
        let before = self.tptypes().ok();

        // Same signed-length trap as `tpalloc`: a `usize` past `c_long::MAX`
        // wraps negative, and the buffer that comes back is not the size that
        // was asked for.
        let c_new_size = c_long::try_from(new_size).map_err(|_| {
            AtmiError::new(
                raw::TPEINVAL,
                format!("size {new_size} exceeds the largest XATMI buffer length"),
            )
        })?;

        if let Some(info) = &before {
            if info.type_name == "VIEW" {
                if let Ok(required) = self.ctx.bvsizeof(&info.subtype) {
                    if new_size < required {
                        return Err(AtmiError::new(
                            raw::TPEINVAL,
                            format!(
                                "cannot shrink view {} to {new_size} bytes, its layout needs {required}",
                                info.subtype
                            ),
                        ));
                    }
                }
            }
        }

        #[cfg(not(feature = "ctx-send"))]
        let new_ptr = unsafe { raw::tprealloc(self.ptr as *mut c_char, c_new_size) };

        #[cfg(feature = "ctx-send")]
        let new_ptr = unsafe { raw::Otprealloc(self.ctx.c_ctx_ptr(), self.ptr, c_new_size) };

        if new_ptr.is_null() {
            Err(self.ctx.atmi_last_error())
        } else {
            self.ptr = new_ptr;

            // Growth exposes fresh uninitialised bytes. Clear them for the same
            // reason `tpalloc` does, so that stating a larger length can never
            // publish uninitialised memory through `as_bytes`.
            //
            // The extent comes from the reallocated buffer, not from
            // `new_size`: the CARRAY allocator applies its own minimum, so the
            // result can be larger than was asked for and the tail beyond the
            // request would otherwise stay dirty.
            let grown = self.tptypes().map(|info| info.size).unwrap_or(0);

            if let Some(info) = &before {
                if info.type_name == "CARRAY" && grown > info.size {
                    unsafe {
                        std::ptr::write_bytes(
                            (self.ptr as *mut u8).add(info.size),
                            0,
                            grown - info.size,
                        )
                    };
                }
            }

            // Shrinking leaves the recorded length past the end of the new
            // allocation. Truncate rather than leave a length that would slice
            // out of bounds.
            self.len = self.len.min(new_size);
            Ok(())
        }
    }
}

/// Free the native allocation when this wrapper owns it.
impl<'ctx> Drop for TypedBuffer<'ctx> {
    /// Free the native allocation when this wrapper owns it.
    fn drop(&mut self) {
        if self.owned && !self.ptr.is_null() {
            #[cfg(not(feature = "ctx-send"))]
            unsafe {
                raw::tpfree(self.ptr)
            }

            #[cfg(feature = "ctx-send")]
            unsafe {
                raw::Otpfree(self.ctx.c_ctx_ptr(), self.ptr)
            }
        }
    }
}
