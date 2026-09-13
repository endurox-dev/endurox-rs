//! UBF occurrence search and last-occurrence access.
// Native long widths vary by target.
#![allow(clippy::unnecessary_cast)]
use crate::{
    raw, AtmiCtx, BorrowedBuffer, BorrowedUbf, TypedBuffer, TypedUbf, TypedView, UbfError,
    UbfFieldType, UbfResult, UbfValue,
};
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::raw::{c_char, c_long};

/// Read-only VIEW storage borrowed from an inline UBF field.
/// The parent cannot be changed or dropped while this view is in use.
pub struct BorrowedView<'a, 'ctx> {
    inner: TypedView<'ctx>,
    _borrow: PhantomData<&'a ()>,
}

impl<'ctx> Deref for BorrowedView<'_, 'ctx> {
    type Target = TypedView<'ctx>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A native field value borrowed by [`TypedUbf::bfindlast`].
/// Fixed-width scalars are copied; variable-sized data remains borrowed.
pub enum UbfFieldRef<'a, 'ctx> {
    Short(i16),
    Long(i64),
    Char(i8),
    Float(f32),
    Double(f64),
    String(&'a CStr),
    Carray(&'a [u8]),
    Ubf(BorrowedUbf<'a, 'ctx>),
    /// `None` represents a native NULL pointer occurrence.
    Ptr(Option<BorrowedBuffer<'a, 'ctx>>),
    /// `None` represents a native NULL VIEW occurrence.
    View(Option<BorrowedView<'a, 'ctx>>),
}

impl<'ctx> TypedUbf<'ctx> {
    /// Find the first occurrence matching `value`, without native type conversion.
    ///
    /// The value variant must match the field type. `regex` selects native full
    /// string regular-expression matching and is valid only for STRING fields;
    /// otherwise comparison is exact. CARRAY lengths include embedded zero bytes.
    /// The supplied value remains owned by the caller. No match returns `BNOTPRES`.
    pub fn bfindocc(&self, field: i32, value: &UbfValue<'_>, regex: bool) -> UbfResult<i32> {
        let kind = self.ctx().bfldtype(field)?;
        if regex && kind != UbfFieldType::String {
            return Err(UbfError::new(
                UbfError::BEINVAL,
                "regex requires a STRING field",
            ));
        }
        let string;
        let long;
        let pointer;
        let mut view: raw::BVIEWFLD = unsafe { std::mem::zeroed() };
        let (data, len, wanted) = match value {
            UbfValue::Short(v) => ((v as *const i16).cast(), 0, UbfFieldType::Short),
            UbfValue::Long(v) => {
                long = c_long::try_from(*v)
                    .map_err(|_| UbfError::new(UbfError::BEINVAL, "value exceeds native long"))?;
                ((&long as *const c_long).cast(), 0, UbfFieldType::Long)
            }
            UbfValue::Char(v) => ((v as *const i8).cast(), 0, UbfFieldType::Char),
            UbfValue::Float(v) => ((v as *const f32).cast(), 0, UbfFieldType::Float),
            UbfValue::Double(v) => ((v as *const f64).cast(), 0, UbfFieldType::Double),
            UbfValue::String(v) => {
                string = CString::new(v.as_str())
                    .map_err(|_| UbfError::new(UbfError::BEINVAL, "search string contains NUL"))?;
                (string.as_ptr(), i32::from(regex), UbfFieldType::String)
            }
            UbfValue::Carray(v) => {
                let len = i32::try_from(v.len()).map_err(|_| {
                    UbfError::new(UbfError::BEINVAL, "search value exceeds BFLDLEN")
                })?;
                (v.as_ptr().cast(), len, UbfFieldType::Carray)
            }
            UbfValue::Ptr(v) => {
                pointer = v.as_ptr();
                (
                    (&pointer as *const *mut c_char).cast(),
                    0,
                    UbfFieldType::Ptr,
                )
            }
            UbfValue::Ubf(v) => (v.as_ptr() as *const c_char, 0, UbfFieldType::Ubf),
            UbfValue::View(v) => {
                crate::write_fixed_str(&mut view.vname, v.bvname(), "VIEW name")
                    .map_err(|e| UbfError::new(UbfError::BEINVAL, e.message))?;
                view.data = v.buffer().as_ptr();
                (
                    (&view as *const raw::BVIEWFLD).cast(),
                    0,
                    UbfFieldType::View,
                )
            }
        };
        if wanted != kind {
            return Err(UbfError::new(
                UbfError::BTYPERR,
                format!("search value {wanted:?} does not match field {kind:?}"),
            ));
        }
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bfindocc(self.as_ubfh(), field, data as *mut c_char, len) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBfindocc(
                self.ctx().c_ctx_ptr(),
                self.as_ubfh(),
                field,
                data as *mut c_char,
                len,
            )
        };
        if rc < 0 {
            Err(self.ctx().ubf_last_error())
        } else {
            Ok(rc)
        }
    }

    /// Borrow the last occurrence and return its index and value.
    ///
    /// STRING, CARRAY and complex storage borrows this UBF; mutation and freeing
    /// are excluded by that borrow. VIEW's native TLS descriptor is copied before
    /// further native calls, so subsequent lookups do not invalidate this view.
    /// An absent field returns `BNOTPRES`.
    ///
    /// ```compile_fail
    /// # use endurox_rs::{AtmiCtx, UbfFieldRef, ubf_fields::T_CARRAY_FLD};
    /// # let ctx = AtmiCtx::new().unwrap();
    /// let mut ubf = ctx.tpalloc_ubf(1024).unwrap();
    /// let (_, value) = ubf.bfindlast(T_CARRAY_FLD).unwrap();
    /// ubf.tprealloc(4096).unwrap();
    /// if let UbfFieldRef::Carray(bytes) = value { println!("{bytes:?}"); }
    /// ```
    pub fn bfindlast(&self, field: i32) -> UbfResult<(i32, UbfFieldRef<'_, 'ctx>)> {
        let kind = self.ctx().bfldtype(field)?;
        let (mut occurrence, mut len) = (0, 0);
        #[cfg(not(feature = "ctx-send"))]
        let data = unsafe { raw::Bfindlast(self.as_ubfh(), field, &mut occurrence, &mut len) };
        #[cfg(feature = "ctx-send")]
        let data = unsafe {
            raw::OBfindlast(
                self.ctx().c_ctx_ptr(),
                self.as_ubfh(),
                field,
                &mut occurrence,
                &mut len,
            )
        };
        if data.is_null() {
            return Err(self.ctx().ubf_last_error());
        }
        let len = usize::try_from(len)
            .map_err(|_| UbfError::new(UbfError::BEINVAL, "negative native field length"))?;
        // All pointers below borrow the immutably borrowed parent. Native field
        // alignment may differ from Rust's, so scalar reads are unaligned.
        let value = unsafe {
            match kind {
                UbfFieldType::Short => UbfFieldRef::Short(data.cast::<i16>().read_unaligned()),
                UbfFieldType::Long => {
                    UbfFieldRef::Long(data.cast::<c_long>().read_unaligned() as i64)
                }
                UbfFieldType::Char => UbfFieldRef::Char(data.cast::<i8>().read_unaligned()),
                UbfFieldType::Float => UbfFieldRef::Float(data.cast::<f32>().read_unaligned()),
                UbfFieldType::Double => UbfFieldRef::Double(data.cast::<f64>().read_unaligned()),
                UbfFieldType::String => {
                    let bytes = std::slice::from_raw_parts(data.cast::<u8>(), len);
                    UbfFieldRef::String(
                        CStr::from_bytes_until_nul(bytes).map_err(|_| {
                            UbfError::new(UbfError::BEINVAL, "unterminated UBF string")
                        })?,
                    )
                }
                UbfFieldType::Carray => {
                    UbfFieldRef::Carray(std::slice::from_raw_parts(data.cast(), len))
                }
                UbfFieldType::Ubf => {
                    UbfFieldRef::Ubf(BorrowedUbf::from_raw(self.ctx(), data.cast()))
                }
                UbfFieldType::Ptr => {
                    let pointer = data.cast::<*mut c_char>().read_unaligned();
                    UbfFieldRef::Ptr(if pointer.is_null() {
                        None
                    } else {
                        Some(BorrowedBuffer::from_unowned(
                            TypedBuffer::borrowed_from_raw(self.ctx(), pointer),
                        ))
                    })
                }
                UbfFieldType::View => {
                    let descriptor = data.cast::<raw::BVIEWFLD>().read_unaligned();
                    let name = CStr::from_ptr(descriptor.vname.as_ptr())
                        .to_str()
                        .map_err(|_| UbfError::new(UbfError::BBADVIEW, "invalid VIEW name"))?;
                    UbfFieldRef::View(if name.is_empty() {
                        None
                    } else {
                        let size = self.ctx().bvsizeof(name)?;
                        if descriptor.data.is_null() || len < size {
                            return Err(UbfError::new(UbfError::BBADVIEW, "short embedded VIEW"));
                        }
                        Some(BorrowedView {
                            inner: TypedView::borrowed_from_raw(
                                self.ctx(),
                                name.to_owned(),
                                descriptor.data,
                            ),
                            _borrow: PhantomData,
                        })
                    })
                }
            }
        };
        Ok((occurrence, value))
    }

    /// Copy the last occurrence and return its index and an independently owned value.
    ///
    /// Complex UBF and pointer targets are deep-copied, including owned descendants.
    /// Absent fields and native NULL PTR/VIEW occurrences return `BNOTPRES`.
    pub fn bgetlast(&self, field: i32) -> UbfResult<(i32, UbfValue<'ctx>)> {
        let mut occurrence = 0;
        // Ask Bgetlast to select the occurrence without a raw destination. Its
        // public implementation supports this. Typed reads below provide owned
        // data instead of shallow C copies of complex pointer graphs.
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bgetlast(
                self.as_ubfh(),
                field,
                &mut occurrence,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBgetlast(
                self.ctx().c_ctx_ptr(),
                self.as_ubfh(),
                field,
                &mut occurrence,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc < 0 {
            return Err(self.ctx().ubf_last_error());
        }
        let value = match self.ctx().bfldtype(field)? {
            UbfFieldType::Short => UbfValue::Short(self.bget_short(field, occurrence)?),
            UbfFieldType::Long => UbfValue::Long(self.bget_long(field, occurrence)?),
            UbfFieldType::Char => UbfValue::Char(self.bget_char(field, occurrence)?),
            UbfFieldType::Float => UbfValue::Float(self.bget_float(field, occurrence)?),
            UbfFieldType::Double => UbfValue::Double(self.bget_double(field, occurrence)?),
            UbfFieldType::String => UbfValue::String(self.bget_string(field, occurrence)?),
            UbfFieldType::Carray => UbfValue::Carray(self.bget_bytes(field, occurrence)?),
            UbfFieldType::Ptr => UbfValue::Ptr(self.clone_pointer_target(field, occurrence)?),
            UbfFieldType::View => UbfValue::View(self.bget_view(field, occurrence)?),
            UbfFieldType::Ubf => {
                let child = self.bget_ubf(field, occurrence)?;
                let child =
                    unsafe { TypedUbf::borrowed_from_raw(self.ctx(), child.as_ubfh().cast()) };
                UbfValue::Ubf(child.deep_clone()?)
            }
        };
        Ok((occurrence, value))
    }
}

impl AtmiCtx {
    /// Estimate UBF allocation bytes for positive field count and total value bytes.
    /// Reject values outside native `BFLDOCC`/`BFLDLEN` ranges before calling C.
    pub fn bneeded(&self, nrfields: usize, totsize: usize) -> UbfResult<usize> {
        let fields = i32::try_from(nrfields)
            .map_err(|_| UbfError::new(UbfError::BEINVAL, "field count exceeds BFLDOCC"))?;
        let bytes = i32::try_from(totsize)
            .map_err(|_| UbfError::new(UbfError::BEINVAL, "value size exceeds BFLDLEN"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bneeded(fields, bytes) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBneeded(self.c_ctx_ptr(), fields, bytes) };
        if rc < 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(rc as usize)
        }
    }
}
