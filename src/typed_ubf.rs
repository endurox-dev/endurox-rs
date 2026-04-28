// src/typed_ubf.rs
use core::ffi::{c_char, c_int};
use std::ffi::{CStr, CString};
use std::ops::{Deref, DerefMut};

use crate::{
    raw, AtmiCtx, AtmiError, BFldLocInfo, TypedBuffer, UbfError, UbfExprTree, UbfFieldType,
    UbfResult,
};

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

/// Value read dynamically from a UBF field based on the field id type.
pub enum UbfGetValue<'a, 'ctx> {
    Short(i16),
    Long(i64),
    Char(i8),
    Float(f32),
    Double(f64),
    String(String),
    Carray(Vec<u8>),
    Ubf(BorrowedUbf<'a, 'ctx>),
}

/// Converts ordinary Rust values into values accepted by UBF write methods.
pub trait IntoUbfValue<'ctx> {
    fn into_ubf_value(self) -> UbfValue<'ctx>;
}

impl<'ctx> IntoUbfValue<'ctx> for UbfValue<'ctx> {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        self
    }
}

impl<'ctx> IntoUbfValue<'ctx> for i16 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Short(self)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for i64 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Long(self)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for isize {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Long(self as i64)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for i32 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Long(self as i64)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for u64 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Long(self as i64)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for usize {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Long(self as i64)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for u32 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Long(self as i64)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for u16 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Short(self as i16)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for u8 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Short(self as i16)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for i8 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Char(self)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for f32 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Float(self)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for f64 {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Double(self)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for String {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::String(self)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for &str {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::String(self.to_string())
    }
}

impl<'ctx> IntoUbfValue<'ctx> for Vec<u8> {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Carray(self)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for TypedBuffer<'ctx> {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Ptr(self)
    }
}

impl<'ctx> IntoUbfValue<'ctx> for TypedUbf<'ctx> {
    fn into_ubf_value(self) -> UbfValue<'ctx> {
        UbfValue::Ubf(self)
    }
}

/// UBF-typed buffer: logically a UBF atmibuf.
#[derive(Debug)]
pub struct TypedUbf<'ctx> {
    inner: TypedBuffer<'ctx>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UbfField {
    pub field_id: i32,
    pub occurrence: i32,
    pub field_type: UbfFieldType,
    pub len: usize,
}

pub struct UbfIterator<'a, 'ctx> {
    ubf: &'a TypedUbf<'ctx>,
    field_id: raw::BFLDID,
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
        let mut buf = vec![0u8; raw::NDRX_ATMI_MSG_MAX_SIZE as usize];
        let mut len = buf.len() as raw::BFLDLEN;
        let rc = self.ctx.cbget_borrowed_ubf_value(
            self,
            bfldid as raw::BFLDID,
            occ as raw::BFLDOCC,
            buf.as_mut_ptr() as *mut c_char,
            &mut len,
            raw::BFLD_STRING as c_int,
        );
        if rc != 0 {
            return Err(self.ctx.ubf_last_error());
        }
        let s = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) }
            .to_string_lossy()
            .into_owned();
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

    /// # Safety
    /// `raw` must be a valid UBF pointer owned by another party.
    pub(crate) unsafe fn borrowed_from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        TypedUbf {
            inner: TypedBuffer::borrowed_from_raw(ctx, raw),
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
        self.inner.ctx.bsizeof(self)
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
        v: impl IntoUbfValue<'ctx>,
        realloc: bool,
    ) -> UbfResult<()> {
        self.write_value(bfldid, occ, v.into_ubf_value(), realloc, false)
    }

    /// Add a new UBF field occurrence.
    pub fn badd(
        &mut self,
        bfldid: i32,
        v: impl IntoUbfValue<'ctx>,
        realloc: bool,
    ) -> UbfResult<()> {
        self.write_value(bfldid, 0, v.into_ubf_value(), realloc, true)
    }

    /// Add a new UBF field occurrence using Enduro/X fast-add location state.
    pub fn badd_fast(
        &mut self,
        bfldid: i32,
        v: impl IntoUbfValue<'ctx>,
        loc: &mut BFldLocInfo,
        first: bool,
        realloc: bool,
    ) -> UbfResult<()> {
        if first {
            *loc = BFldLocInfo::default();
        }
        self.write_value_fast(bfldid, v.into_ubf_value(), loc, realloc)
    }

    /// Change or add a field based on `do_add`, matching Go's `BChgCombined`.
    pub fn bchg_combined(
        &mut self,
        bfldid: i32,
        occ: i32,
        v: impl IntoUbfValue<'ctx>,
        do_add: bool,
        realloc: bool,
    ) -> UbfResult<()> {
        self.write_value(bfldid, occ, v.into_ubf_value(), realloc, do_add)
    }

    pub fn bnext(&self) -> UbfIterator<'_, 'ctx> {
        UbfIterator {
            ubf: self,
            field_id: 0,
        }
    }

    fn write_value(
        &mut self,
        bfldid: i32,
        occ: i32,
        mut v: UbfValue<'ctx>,
        realloc: bool,
        add: bool,
    ) -> UbfResult<()> {
        loop {
            if let UbfValue::Ubf(ubf) = &mut v {
                if add {
                    return Err(UbfError::new(
                        UbfError::BEINVAL,
                        "adding embedded UBF fields is not supported",
                    ));
                }
                let rc = self.inner.ctx.bchg_ubf_value(
                    self,
                    bfldid as raw::BFLDID,
                    occ as raw::BFLDOCC,
                    ubf,
                );

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

            let mut _string_storage: Option<CString> = None;
            let mut empty_carray = [0u8; 1];

            let (ptr, len, ftype) = match &mut v {
                UbfValue::Short(val) => (val as *mut i16 as *mut c_char, 0, raw::BFLD_SHORT),
                UbfValue::Long(val) => (val as *mut i64 as *mut c_char, 0, raw::BFLD_LONG),
                UbfValue::Char(val) => (val as *mut i8 as *mut c_char, 0, raw::BFLD_CHAR),
                UbfValue::Float(val) => (val as *mut f32 as *mut c_char, 0, raw::BFLD_FLOAT),
                UbfValue::Double(val) => (val as *mut f64 as *mut c_char, 0, raw::BFLD_DOUBLE),
                UbfValue::String(s) => {
                    let cstr = CString::new(s.as_str())
                        .map_err(|e| UbfError::new(UbfError::BEUNIX, e.to_string()))?;
                    let p = cstr.as_ptr() as *mut c_char;
                    _string_storage = Some(cstr);
                    (p, 0, raw::BFLD_STRING)
                }
                UbfValue::Carray(v) => {
                    let p = if v.is_empty() {
                        empty_carray.as_mut_ptr() as *mut c_char
                    } else {
                        v.as_mut_ptr() as *mut c_char
                    };
                    (p, v.len() as raw::BFLDLEN, raw::BFLD_CARRAY)
                }
                UbfValue::Ptr(buf) => (buf.as_ptr() as *mut c_char, 0, raw::BFLD_PTR),
                UbfValue::Ubf(_) => unreachable!("handled before typed write"),
            };

            let rc = if add {
                self.inner
                    .ctx
                    .cbadd_value(self, bfldid as raw::BFLDID, ptr, len, ftype as c_int)
            } else {
                self.inner.ctx.cbchg_value(
                    self,
                    bfldid as raw::BFLDID,
                    occ as raw::BFLDOCC,
                    ptr,
                    len,
                    ftype as c_int,
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
    }

    fn write_value_fast(
        &mut self,
        bfldid: i32,
        mut v: UbfValue<'ctx>,
        loc: &mut BFldLocInfo,
        realloc: bool,
    ) -> UbfResult<()> {
        loop {
            let mut _string_storage: Option<CString> = None;
            let mut empty_carray = [0u8; 1];

            let (ptr, len, ftype) = match &mut v {
                UbfValue::Short(val) => (val as *mut i16 as *mut c_char, 0, raw::BFLD_SHORT),
                UbfValue::Long(val) => (val as *mut i64 as *mut c_char, 0, raw::BFLD_LONG),
                UbfValue::Char(val) => (val as *mut i8 as *mut c_char, 0, raw::BFLD_CHAR),
                UbfValue::Float(val) => (val as *mut f32 as *mut c_char, 0, raw::BFLD_FLOAT),
                UbfValue::Double(val) => (val as *mut f64 as *mut c_char, 0, raw::BFLD_DOUBLE),
                UbfValue::String(s) => {
                    let cstr = CString::new(s.as_str())
                        .map_err(|e| UbfError::new(UbfError::BEUNIX, e.to_string()))?;
                    let p = cstr.as_ptr() as *mut c_char;
                    _string_storage = Some(cstr);
                    (p, 0, raw::BFLD_STRING)
                }
                UbfValue::Carray(v) => {
                    let p = if v.is_empty() {
                        empty_carray.as_mut_ptr() as *mut c_char
                    } else {
                        v.as_mut_ptr() as *mut c_char
                    };
                    (p, v.len() as raw::BFLDLEN, raw::BFLD_CARRAY)
                }
                UbfValue::Ptr(buf) => (buf.as_ptr() as *mut c_char, 0, raw::BFLD_PTR),
                UbfValue::Ubf(_) => {
                    return Err(UbfError::new(
                        UbfError::BEINVAL,
                        "fast-add embedded UBF fields is not supported",
                    ))
                }
            };

            let rc = self.inner.ctx.baddfast_value(
                self,
                bfldid as raw::BFLDID,
                ptr,
                len,
                ftype as c_int,
                loc,
            );

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
    }

    // --- Safe typed field getters -------------------------------------------

    /// Read a UBF field occurrence as a `String`.
    ///
    /// Uses `CBget(3)` so Enduro/X performs type conversion from the stored
    /// field type to `BFLD_STRING`.
    pub fn bget_string(&self, bfldid: i32, occ: i32) -> UbfResult<String> {
        let mut buf = vec![0u8; raw::NDRX_ATMI_MSG_MAX_SIZE as usize];
        let mut len = buf.len() as raw::BFLDLEN;
        let rc = self.inner.ctx.cbget_value(
            self,
            bfldid as raw::BFLDID,
            occ as raw::BFLDOCC,
            buf.as_mut_ptr() as *mut c_char,
            &mut len,
            raw::BFLD_STRING as c_int,
        );
        if rc != 0 {
            return Err(self.inner.ctx.ubf_last_error());
        }
        let s = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) }
            .to_string_lossy()
            .into_owned();
        Ok(s)
    }

    /// Read a field occurrence dynamically based on the UBF field id type.
    pub fn bget<'a>(&'a self, bfldid: i32, occ: i32) -> UbfResult<UbfGetValue<'a, 'ctx>> {
        match self.inner.ctx.bfldtype(bfldid as raw::BFLDID)? {
            UbfFieldType::Short => Ok(UbfGetValue::Short(self.bget_short(bfldid, occ)?)),
            UbfFieldType::Long => Ok(UbfGetValue::Long(self.bget_long(bfldid, occ)?)),
            UbfFieldType::Char => Ok(UbfGetValue::Char(self.bget_char(bfldid, occ)?)),
            UbfFieldType::Float => Ok(UbfGetValue::Float(self.bget_float(bfldid, occ)?)),
            UbfFieldType::Double => Ok(UbfGetValue::Double(self.bget_double(bfldid, occ)?)),
            UbfFieldType::String => Ok(UbfGetValue::String(self.bget_string(bfldid, occ)?)),
            UbfFieldType::Carray => Ok(UbfGetValue::Carray(self.bget_bytes(bfldid, occ)?)),
            UbfFieldType::Ubf => Ok(UbfGetValue::Ubf(self.bget_ubf(bfldid, occ)?)),
            UbfFieldType::Ptr | UbfFieldType::View => Err(UbfError::new(
                UbfError::BEINVAL,
                "dynamic Bget for ptr/view fields is not supported",
            )),
        }
    }

    /// Read a UBF field occurrence as an `i64`.
    ///
    /// Uses `CBget(3)` with `BFLD_LONG` as the requested target type.
    pub fn bget_long(&self, bfldid: i32, occ: i32) -> UbfResult<i64> {
        let mut val: i64 = 0;
        let mut len = std::mem::size_of::<i64>() as raw::BFLDLEN;
        let rc = self.inner.ctx.cbget_value(
            self,
            bfldid as raw::BFLDID,
            occ as raw::BFLDOCC,
            &mut val as *mut i64 as *mut c_char,
            &mut len,
            raw::BFLD_LONG as c_int,
        );
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
        let rc = self.inner.ctx.cbget_value(
            self,
            bfldid as raw::BFLDID,
            occ as raw::BFLDOCC,
            &mut val as *mut i16 as *mut c_char,
            &mut len,
            raw::BFLD_SHORT as c_int,
        );
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
        let rc = self.inner.ctx.cbget_value(
            self,
            bfldid as raw::BFLDID,
            occ as raw::BFLDOCC,
            &mut val as *mut f64 as *mut c_char,
            &mut len,
            raw::BFLD_DOUBLE as c_int,
        );
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
        let rc = self.inner.ctx.cbget_value(
            self,
            bfldid as raw::BFLDID,
            occ as raw::BFLDOCC,
            &mut val as *mut f32 as *mut c_char,
            &mut len,
            raw::BFLD_FLOAT as c_int,
        );
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
        let rc = self.inner.ctx.cbget_value(
            self,
            bfldid as raw::BFLDID,
            occ as raw::BFLDOCC,
            &mut val as *mut i8 as *mut c_char,
            &mut len,
            raw::BFLD_CHAR as c_int,
        );
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
        let mut buf = vec![0u8; raw::NDRX_ATMI_MSG_MAX_SIZE as usize];
        let mut len = buf.len() as raw::BFLDLEN;
        let rc = self.inner.ctx.cbget_value(
            self,
            bfldid as raw::BFLDID,
            occ as raw::BFLDOCC,
            buf.as_mut_ptr() as *mut c_char,
            &mut len,
            raw::BFLD_CARRAY as c_int,
        );
        if rc != 0 {
            return Err(self.inner.ctx.ubf_last_error());
        }
        buf.truncate(len as usize);
        let bytes = buf;
        Ok(bytes)
    }

    /// Read an embedded UBF occurrence as a borrowed read-only UBF view.
    ///
    /// The returned view is tied to this parent buffer and must not outlive it.
    pub fn bget_ubf<'a>(&'a self, bfldid: i32, occ: i32) -> UbfResult<BorrowedUbf<'a, 'ctx>> {
        let mut len: raw::BFLDLEN = 0;
        let ptr =
            self.inner
                .ctx
                .bfind_value(self, bfldid as raw::BFLDID, occ as raw::BFLDOCC, &mut len);
        if ptr.is_null() {
            return Err(self.inner.ctx.ubf_last_error());
        }
        Ok(unsafe { BorrowedUbf::from_raw(self.inner.ctx, ptr as *mut raw::UBFH) })
    }

    /// Evaluate a compiled boolean expression against this UBF.
    pub fn bboolev(&self, tree: &UbfExprTree<'_>) -> bool {
        self.inner.ctx.bboolev_value(self, tree) == 1
    }

    /// Compile and evaluate a boolean expression against this UBF.
    pub fn bqboolev(&self, expr: &str) -> UbfResult<bool> {
        let tree = self.inner.ctx.bboolco(expr)?;
        Ok(self.bboolev(&tree))
    }

    /// Evaluate a compiled expression as a floating point value.
    pub fn bfloatev(&self, tree: &UbfExprTree<'_>) -> f64 {
        self.inner.ctx.bfloatev_value(self, tree)
    }

    /// Print this UBF buffer to stdout.
    pub fn bprint(&self) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bprint(self.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBprint(self.inner.ctx.c_ctx_ptr(), self.as_ubfh()) };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.inner.ctx.ubf_last_error())
        }
    }

    /// Print this UBF buffer to Enduro/X logs.
    pub fn tplogprintubf(&self, level: i32, title: &str) -> UbfResult<()> {
        let title =
            CString::new(title).map_err(|e| UbfError::new(UbfError::BEINVAL, e.to_string()))?;

        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::tplogprintubf(level as c_int, title.as_ptr(), self.as_ubfh())
        };

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::Otplogprintubf(
                self.inner.ctx.c_ctx_ptr(),
                level as c_int,
                title.as_ptr(),
                self.as_ubfh(),
            )
        };

        Ok(())
    }

    /// Print this UBF buffer to a string using callback I/O.
    pub fn bsprint(&self) -> UbfResult<String> {
        self.inner.ctx.bfprintcb_value(self)
    }

    /// Read textual `Bprint`/`Bextread` format into this UBF.
    pub fn bextread(&mut self, text: &str) -> UbfResult<()> {
        self.inner.ctx.bextreadcb_value(self, text)
    }

    /// Serialize this UBF buffer to bytes.
    pub fn bwrite(&self) -> UbfResult<Vec<u8>> {
        self.inner.ctx.bwritecb_value(self)
    }

    /// Read serialized UBF bytes into this buffer.
    pub fn bread(&mut self, dump: &[u8]) -> UbfResult<()> {
        self.inner.ctx.breadcb_value(self, dump)
    }
} // impl TypedUbf

impl<'a, 'ctx> UbfIterator<'a, 'ctx> {
    pub fn next(&mut self) -> UbfResult<Option<UbfField>> {
        let mut occurrence: raw::BFLDOCC = 0;
        let rc = self
            .ubf
            .inner
            .ctx
            .bnext_value(self.ubf, &mut self.field_id, &mut occurrence);

        match rc {
            1 => {
                let field_type = self.ubf.inner.ctx.bfldtype(self.field_id)?;
                let len = self
                    .ubf
                    .inner
                    .ctx
                    .blen(self.ubf, self.field_id, occurrence)?;
                Ok(Some(UbfField {
                    field_id: self.field_id as i32,
                    occurrence,
                    field_type,
                    len,
                }))
            }
            0 => Ok(None),
            _ => Err(self.ubf.inner.ctx.ubf_last_error()),
        }
    }
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
