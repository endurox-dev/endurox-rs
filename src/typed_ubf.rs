// src/typed_ubf.rs
use core::ffi::{c_char, c_int, c_long};
use std::ffi::CStr;
use std::ops::{Deref, DerefMut};

use crate::{raw, AtmiCtx, AtmiError, TypedBuffer, UbfError, UbfResult};

/// Value that can be written into a UBF field.
///
/// The variant should match the destination field type. When possible, `bchg`
/// uses Enduro/X typed conversion APIs so Rust callers can work with ordinary
/// Rust values.
pub enum UbfValue<'ctx> {
    /// Signed 16-bit integer field.
    Short(i16),
    /// Signed long integer field.
    Long(i64),
    /// Single byte character field.
    Char(i8),
    /// 32-bit floating point field.
    Float(f32),
    /// 64-bit floating point field.
    Double(f64),
    /// NUL-terminated string field. Interior NUL bytes are rejected.
    String(String),
    /// Byte array field. Length is preserved.
    Carray(Vec<u8>),
    /// Pointer field containing another typed ATMI buffer.
    Ptr(TypedBuffer<'ctx>),
    /// Embedded UBF field.
    Ubf(TypedUbf<'ctx>), //Ubf(TypedView<'ctx>) - TODO
}

/// UBF-typed buffer: logically a UBF atmibuf.
#[derive(Debug)]
pub struct TypedUbf<'ctx> {
    inner: TypedBuffer<'ctx>,
}

/// Borrowed read-only view of an embedded UBF field.
///
/// This does not own the underlying buffer and must not free it. Its lifetime is
/// tied to the parent UBF borrowed by [`TypedUbf::bget_ubf`].
#[derive(Debug)]
pub struct BorrowedUbf<'a, 'ctx> {
    ptr: *mut raw::UBFH,
    ctx: &'ctx AtmiCtx,
    _borrow: std::marker::PhantomData<&'a raw::UBFH>,
}

impl<'a, 'ctx> BorrowedUbf<'a, 'ctx> {
    /// # Safety
    /// `ptr` must point to a valid embedded UBF field owned by another UBF
    /// buffer that outlives `'a`.
    pub(crate) unsafe fn from_raw(ctx: &'ctx AtmiCtx, ptr: *mut raw::UBFH) -> Self {
        Self {
            ptr,
            ctx,
            _borrow: std::marker::PhantomData,
        }
    }

    #[inline]
    pub(crate) fn as_ubfh(&self) -> *mut raw::UBFH {
        self.ptr
    }

    /// Read a field from this embedded UBF as a `String`.
    pub fn bget_string(&self, bfldid: i32, occ: i32) -> UbfResult<String> {
        let mut extralen: raw::BFLDLEN = 0;
        let ptr = unsafe {
            self.ctx.cbgetalloc(
                self.as_ubfh(),
                bfldid as raw::BFLDID,
                occ as raw::BFLDOCC,
                raw::BFLD_STRING as c_int,
                &mut extralen,
            )
        };
        if ptr.is_null() {
            return Err(self.ctx.ubf_last_error());
        }
        let s = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { libc::free(ptr as *mut libc::c_void) };
        Ok(s)
    }
}

impl<'ctx> TypedUbf<'ctx> {
    /// # Safety
    /// `raw` must be a valid UBF (`UBFH*`) allocated for this context.
    pub(crate) unsafe fn from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        TypedUbf {
            inner: TypedBuffer::from_raw(ctx, raw),
        }
    }

    /// Convert a generic typed buffer into a UBF buffer wrapper.
    ///
    /// The caller must know that the wrapped ATMI buffer is actually a UBF
    /// buffer, for example because it came from `tpalloc("UBF", ...)`.
    pub fn from_typed(buf: TypedBuffer<'ctx>) -> Self {
        TypedUbf { inner: buf }
    }

    /// Return the underlying generic typed buffer wrapper.
    pub fn into_inner(self) -> TypedBuffer<'ctx> {
        self.inner
    }

    /// Transfer ownership of the underlying UBF buffer pointer.
    ///
    /// The returned pointer will not be freed by this Rust value. This is mainly
    /// useful when handing the buffer to Enduro/X APIs that take ownership, such
    /// as service forwarding/return paths.
    pub(crate) fn into_raw(self) -> *mut c_char {
        self.inner.into_raw()
    }

    /// UBF header pointer — internal use only.
    #[inline]
    pub(crate) fn as_ubfh(&self) -> *mut raw::UBFH {
        self.inner.as_ptr() as *mut raw::UBFH
    }

    /// # Safety
    /// Move this UBF buffer to a different context.
    ///
    /// Only valid if the C library allows using this buffer under `new_ctx`.
    pub(crate) unsafe fn move_to_context<'new>(self, new_ctx: &'new AtmiCtx) -> TypedUbf<'new> {
        let ptr = self.into_raw();
        TypedUbf::from_raw(new_ctx, ptr)
    }

    /// Return the allocated UBF buffer size in bytes.
    ///
    /// Wraps `Bsizeof(3)`. This reports the buffer allocation size, not the
    /// amount of payload currently used.
    pub fn bsizeof(&mut self) -> UbfResult<usize> {
        let rc = unsafe { raw::Bsizeof(self.inner.as_ptr() as *mut raw::UBFH) };

        if raw::EXFAIL as c_long == rc {
            //Generate error.
            Err(self.ctx.ubf_last_error())
        } else {
            Ok(rc as usize)
        }
    }

    /// Reallocate the buffer twice of the size
    fn grow_buffer(&mut self) -> UbfResult<()> {
        let cur_size = self.bsizeof()?;
        self.inner.tprealloc(cur_size * 2).map_err(|e: AtmiError| {
            // Reuse the message from AtmiError, change the code to BMALLOC
            UbfError::new(UbfError::BMALLOC, e.message.clone())
        })?;
        Ok(())
    }

    /// Change or add a UBF field occurrence.
    ///
    /// Wraps `CBchg(3)` for scalar values and `Bchg(3)` for embedded UBF
    /// values.
    ///
    /// If `realloc` is true and Enduro/X reports `BNOSPACE`, the buffer is
    /// grown and the operation is retried.
    pub fn bchg(
        &mut self,
        bfldid: i32,
        occ: i32,
        mut v: UbfValue<'ctx>,
        realloc: bool,
    ) -> UbfResult<()> {
        use std::ffi::CString;
        use std::os::raw::c_char;

        loop {
            if let UbfValue::Ubf(ubf) = &mut v {
                let rc = unsafe {
                    self.inner.ctx.bchg(
                        self.as_ubfh(),
                        bfldid as raw::BFLDID,
                        occ as raw::BFLDOCC,
                        ubf.as_ubfh() as *mut c_char,
                        0,
                    )
                };

                if rc == 0 {
                    return Ok(());
                }

                let err = self.inner.ctx.ubf_last_error();
                if err.code == UbfError::BNOSPACE && realloc {
                    self.grow_buffer()?;
                    continue;
                }
                return Err(err);
            }

            // Keep owned data (like CString) alive until after CBchg()
            let mut _string_storage: Option<CString> = None;

            // Compute ptr/len/ftype for a single CBchg() call
            let (ptr, len, ftype) = match &mut v {
                UbfValue::Short(val) => {
                    let p = val as *mut i16 as *mut c_char;
                    (p, 0, raw::BFLD_SHORT)
                }
                UbfValue::Long(val) => {
                    let p = val as *mut i64 as *mut c_char;
                    (p, 0, raw::BFLD_LONG)
                }
                UbfValue::Char(val) => {
                    let p = val as *mut i8 as *mut c_char;
                    (p, 0, raw::BFLD_CHAR)
                }
                UbfValue::Float(val) => {
                    let p = val as *mut f32 as *mut c_char;
                    (p, 0, raw::BFLD_FLOAT)
                }
                UbfValue::Double(val) => {
                    let p = val as *mut f64 as *mut c_char;
                    (p, 0, raw::BFLD_DOUBLE)
                }
                UbfValue::String(s) => {
                    // s: &mut String
                    let cstr = CString::new(s.as_str())
                        .map_err(|e| UbfError::new(UbfError::BEUNIX, e.to_string()))?;
                    let p = cstr.as_ptr() as *mut c_char;
                    _string_storage = Some(cstr); // keep it alive for this iteration
                    (p, 0, raw::BFLD_STRING)
                }
                UbfValue::Carray(v) => {
                    // v: &mut Vec<u8>, we don't move it
                    if v.is_empty() {
                        (std::ptr::null_mut(), 0, raw::BFLD_CARRAY)
                    } else {
                        let p = v.as_mut_ptr() as *mut c_char;
                        let len = v.len() as raw::BFLDLEN;
                        (p, len, raw::BFLD_CARRAY)
                    }
                }
                UbfValue::Ptr(buf) => {
                    // buf: &mut TypedBuffer<'ctx>
                    let p = buf.as_ptr() as *mut c_char;
                    (p, 0, raw::BFLD_PTR)
                }
                UbfValue::Ubf(_) => unreachable!("handled before CBchg"),
                // TODO: Add support for view
            };

            // One CBchg() call
            let rc = unsafe {
                raw::CBchg(
                    self.as_ubfh(),
                    bfldid as raw::BFLDID,
                    occ as raw::BFLDOCC,
                    ptr,
                    len,
                    ftype as c_int,
                )
            };

            if rc == 0 {
                return Ok(());
            } else {
                let err = self.inner.ctx.ubf_last_error();

                if err.code == UbfError::BNOSPACE && realloc {
                    // Reallocate the buffer to twice the size and retry.
                    self.grow_buffer()?;
                    continue;
                } else {
                    return Err(err);
                }
            }
        }
    } // bchg()

    // --- Safe typed field getters -------------------------------------------

    /// Read a UBF field occurrence as a `String`.
    ///
    /// Uses `CBgetalloc(3)` so Enduro/X performs type conversion from the
    /// stored field type to `BFLD_STRING`.
    pub fn bget_string(&self, bfldid: i32, occ: i32) -> UbfResult<String> {
        let mut extralen: raw::BFLDLEN = 0;
        let ptr = unsafe {
            self.inner.ctx.cbgetalloc(
                self.as_ubfh(),
                bfldid as raw::BFLDID,
                occ as raw::BFLDOCC,
                raw::BFLD_STRING as c_int,
                &mut extralen,
            )
        };
        if ptr.is_null() {
            return Err(self.inner.ctx.ubf_last_error());
        }
        let s = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { libc::free(ptr as *mut libc::c_void) };
        Ok(s)
    }

    /// Read a UBF field occurrence as an `i64`.
    ///
    /// Uses `CBget(3)` with `BFLD_LONG` as the requested target type.
    pub fn bget_long(&self, bfldid: i32, occ: i32) -> UbfResult<i64> {
        let mut val: i64 = 0;
        let mut len = std::mem::size_of::<i64>() as raw::BFLDLEN;
        let rc = unsafe {
            self.inner.ctx.cbget(
                self.as_ubfh(),
                bfldid as raw::BFLDID,
                occ as raw::BFLDOCC,
                &mut val as *mut i64 as *mut c_char,
                &mut len,
                raw::BFLD_LONG as c_int,
            )
        };
        if rc != 0 {
            Err(self.inner.ctx.ubf_last_error())
        } else {
            Ok(val)
        }
    }

    /// Read a UBF field occurrence as an `i16`.
    ///
    /// Uses `CBget(3)` with `BFLD_SHORT` as the requested target type.
    pub fn bget_short(&self, bfldid: i32, occ: i32) -> UbfResult<i16> {
        let mut val: i16 = 0;
        let mut len = std::mem::size_of::<i16>() as raw::BFLDLEN;
        let rc = unsafe {
            self.inner.ctx.cbget(
                self.as_ubfh(),
                bfldid as raw::BFLDID,
                occ as raw::BFLDOCC,
                &mut val as *mut i16 as *mut c_char,
                &mut len,
                raw::BFLD_SHORT as c_int,
            )
        };
        if rc != 0 {
            Err(self.inner.ctx.ubf_last_error())
        } else {
            Ok(val)
        }
    }

    /// Read a UBF field occurrence as an `f64`.
    ///
    /// Uses `CBget(3)` with `BFLD_DOUBLE` as the requested target type.
    pub fn bget_double(&self, bfldid: i32, occ: i32) -> UbfResult<f64> {
        let mut val: f64 = 0.0;
        let mut len = std::mem::size_of::<f64>() as raw::BFLDLEN;
        let rc = unsafe {
            self.inner.ctx.cbget(
                self.as_ubfh(),
                bfldid as raw::BFLDID,
                occ as raw::BFLDOCC,
                &mut val as *mut f64 as *mut c_char,
                &mut len,
                raw::BFLD_DOUBLE as c_int,
            )
        };
        if rc != 0 {
            Err(self.inner.ctx.ubf_last_error())
        } else {
            Ok(val)
        }
    }

    /// Read a UBF field occurrence as an `f32`.
    ///
    /// Uses `CBget(3)` with `BFLD_FLOAT` as the requested target type.
    pub fn bget_float(&self, bfldid: i32, occ: i32) -> UbfResult<f32> {
        let mut val: f32 = 0.0;
        let mut len = std::mem::size_of::<f32>() as raw::BFLDLEN;
        let rc = unsafe {
            self.inner.ctx.cbget(
                self.as_ubfh(),
                bfldid as raw::BFLDID,
                occ as raw::BFLDOCC,
                &mut val as *mut f32 as *mut c_char,
                &mut len,
                raw::BFLD_FLOAT as c_int,
            )
        };
        if rc != 0 {
            Err(self.inner.ctx.ubf_last_error())
        } else {
            Ok(val)
        }
    }

    /// Read a UBF field occurrence as an `i8`.
    ///
    /// Uses `CBget(3)` with `BFLD_CHAR` as the requested target type.
    pub fn bget_char(&self, bfldid: i32, occ: i32) -> UbfResult<i8> {
        let mut val: i8 = 0;
        let mut len = std::mem::size_of::<i8>() as raw::BFLDLEN;
        let rc = unsafe {
            self.inner.ctx.cbget(
                self.as_ubfh(),
                bfldid as raw::BFLDID,
                occ as raw::BFLDOCC,
                &mut val as *mut i8 as *mut c_char,
                &mut len,
                raw::BFLD_CHAR as c_int,
            )
        };
        if rc != 0 {
            Err(self.inner.ctx.ubf_last_error())
        } else {
            Ok(val)
        }
    }

    /// Read a UBF `BFLD_CARRAY` occurrence into an owned byte vector.
    ///
    /// The exact CARRAY length returned by Enduro/X is preserved.
    pub fn bget_bytes(&self, bfldid: i32, occ: i32) -> UbfResult<Vec<u8>> {
        let mut extralen: raw::BFLDLEN = 0;
        let ptr = unsafe {
            self.inner.ctx.cbgetalloc(
                self.as_ubfh(),
                bfldid as raw::BFLDID,
                occ as raw::BFLDOCC,
                raw::BFLD_CARRAY as c_int,
                &mut extralen,
            )
        };
        if ptr.is_null() {
            return Err(self.inner.ctx.ubf_last_error());
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(ptr as *const u8, extralen as usize) }.to_vec();
        unsafe { libc::free(ptr as *mut libc::c_void) };
        Ok(bytes)
    }

    /// Read an embedded UBF occurrence as a borrowed read-only UBF view.
    ///
    /// The returned view is tied to this parent buffer and must not outlive it.
    pub fn bget_ubf<'a>(&'a self, bfldid: i32, occ: i32) -> UbfResult<BorrowedUbf<'a, 'ctx>> {
        let mut len: raw::BFLDLEN = 0;
        let ptr = unsafe {
            self.inner.ctx.bfind(
                self.as_ubfh(),
                bfldid as raw::BFLDID,
                occ as raw::BFLDOCC,
                &mut len,
            )
        };
        if ptr.is_null() {
            return Err(self.inner.ctx.ubf_last_error());
        }
        Ok(unsafe { BorrowedUbf::from_raw(self.inner.ctx, ptr as *mut raw::UBFH) })
    }
} // impl TypedUbf

impl<'ctx> Deref for TypedUbf<'ctx> {
    type Target = TypedBuffer<'ctx>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'ctx> DerefMut for TypedUbf<'ctx> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
