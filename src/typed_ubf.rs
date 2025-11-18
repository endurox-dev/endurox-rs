// src/typed_ubf.rs
use core::ffi::{c_char, c_long, c_int};
use std::ops::{Deref, DerefMut};

use crate::{raw, AtmiCtx, AtmiError, TypedBuffer, UbfResult, UbfError};

///UBF field value
pub enum UbfValue<'ctx> {

    Short(i16),
    Long(i64),
    Char(i8),
    Float(f32),
    Double(f64),
    String(String),
    Carray(Vec<u8>),
    Ptr(TypedBuffer<'ctx>),
    Ubf(TypedUbf<'ctx>)
    //Ubf(TypedView<'ctx>) - TODO
}

/// UBF-typed buffer: logically a UBF atmibuf.
#[derive(Debug)]
pub struct TypedUbf<'ctx> {
    inner: TypedBuffer<'ctx>,
}

impl<'ctx> TypedUbf<'ctx> {
    /// # Safety
    /// `raw` must be a valid UBF (`UBFH*`) allocated for this context.
    pub unsafe fn from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        TypedUbf { inner: TypedBuffer::from_raw(ctx, raw) }
    }

    /// Blind cast from a generic buffer you know is UBF.
    pub fn from_typed(buf: TypedBuffer<'ctx>) -> Self {
        TypedUbf { inner: buf }
    }

    /// Give up this wrapper and return the underlying `TypedBuffer`.
    pub fn into_inner(self) -> TypedBuffer<'ctx> {
        self.inner
    }

    /// Transfer ownership of the underlying pointer (no Drop).
    pub fn into_raw(self) -> *mut c_char {
        self.inner.into_raw()
    }

    /// UBF header pointer.
    #[inline]
    pub fn as_ubfh(&self) -> *mut raw::UBFH {
        self.inner.as_ptr() as *mut raw::UBFH
    }

    /// # Safety
    /// Move this UBF buffer to a different context.
    ///
    /// Only valid if the C library allows using this buffer under `new_ctx`.
    pub unsafe fn move_to_context<'new>(
        self,
        new_ctx: &'new AtmiCtx,
    ) -> TypedUbf<'new> {
        let ptr = self.into_raw();
        TypedUbf::from_raw(new_ctx, ptr)
    }

    ///Get size of the buffer. See *Bsizeof(3)* for more details
    /// 
    /// # Returns
    ///
    /// * `Ok(size)` – size of the buffer in bytes.
    /// * `Err(e)` – if the underlying `Bsizeof` call fails.
    pub fn bsizeof(&mut self) -> UbfResult<usize> {

        let rc = unsafe {raw::Bsizeof(self.inner.as_ptr() as *mut raw::UBFH)};

        if raw::EXFAIL as c_long ==rc {
            //Generate error.
            Err(self.ctx.ubf_last_error())
        } else {
            Ok(rc as usize)
        }
    }

    /// Reallocate the buffer twice of the size
    fn grow_buffer(&mut self) -> UbfResult<()> {
        let cur_size = self.bsizeof()?;
        self.inner
            .tprealloc(cur_size * 2)
            .map_err(|e: AtmiError| {
                // Reuse the message from AtmiError, change the code to BMALLOC
                UbfError::new(UbfError::BMALLOC, e.message.clone())
            })?;
        Ok(())
    }

    /// Change UBF field value. See Bchg(3) for more details.
    /// 
    /// # Parameters
    ///
    /// * `bfldid` – UBF field id to change.
    /// * `occ` – occurrence index for the field (0-based).
    /// * `v` – value to store at the given field/occurrence in this buffer.
    /// * `realloc` – whether the operation may reallocate the underlying
    ///   buffer in case of getting BNOSPACE error.
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
                UbfValue::Ubf(ubf) => {
                    // ubf: &mut TypedUbf<'ctx>
                    let p = ubf.as_ubfh() as *mut c_char;
                    (p, 0, raw::BFLD_UBF)
                }
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

}

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
