//! Mappings between Rust structures and compiled Enduro/X VIEW layouts.
use crate::ubf_serde::ubf_occurrence;
use crate::{raw, AtmiCtx, TypedView, UbfCarray, UbfError, UbfResult};
use std::{
    ffi::CString,
    os::raw::{c_char, c_int},
};

/// Serialize a named Rust structure into its compiled native VIEW layout.
pub trait ViewSerialize {
    /// Name of the compiled layout, loaded through VIEWFILES / VIEWDIR.
    const VIEW_NAME: &'static str;
    fn view_serialize(&self, view: &mut TypedView<'_>) -> UbfResult<()>;
}
/// Deserialize owned Rust values from a compiled native VIEW layout.
pub trait ViewDeserialize: Sized {
    const VIEW_NAME: &'static str;
    fn view_deserialize(view: &TypedView<'_>) -> UbfResult<Self>;
}
/// Serialize a member to one or more occurrences of a VIEW field.
pub trait ViewFieldSerialize {
    /// Whether this value always occupies one occurrence, including NULLs.
    const SINGLE: bool = true;
    fn view_write_field(
        &self,
        view: &mut TypedView<'_>,
        field: &str,
        occurrence: i32,
    ) -> UbfResult<()>;
}
/// Deserialize a member from one or more occurrences of a VIEW field.
pub trait ViewFieldDeserialize: Sized {
    const SINGLE: bool = true;
    fn view_read_field(view: &TypedView<'_>, field: &str, occurrence: i32) -> UbfResult<Self>;
}

impl TypedView<'_> {
    /// Write mapped members after validating the compiled VIEW name.
    /// An error can leave earlier members updated.
    pub fn view_write<T: ViewSerialize>(&mut self, value: &T) -> UbfResult<()> {
        check_view(self, T::VIEW_NAME)?;
        value.view_serialize(self)
    }
    /// Read a mapped structure without changing the source VIEW.
    pub fn view_read<T: ViewDeserialize>(&self) -> UbfResult<T> {
        check_view(self, T::VIEW_NAME)?;
        T::view_deserialize(self)
    }

    /// Test the compiled NULL sentinel for one occurrence.
    pub fn bvnull(&self, field: &str, occurrence: i32) -> UbfResult<bool> {
        let name = cstring(self.bvname())?;
        let field = cstring(field)?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bvnull(
                self.buffer().as_ptr(),
                field.as_ptr() as *mut c_char,
                occurrence,
                name.as_ptr() as *mut c_char,
            )
        };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBvnull(
                self.buffer().ctx.c_ctx_ptr(),
                self.buffer().as_ptr(),
                field.as_ptr() as *mut c_char,
                occurrence,
                name.as_ptr() as *mut c_char,
            )
        };
        if rc < 0 {
            Err(self.buffer().ctx.ubf_last_error())
        } else {
            Ok(rc != 0)
        }
    }

    /// Set one occurrence to its compiled NULL sentinel. Fails if none exists.
    pub fn bvsetnull(&mut self, field: &str, occurrence: i32) -> UbfResult<()> {
        let template = self
            .buffer()
            .ctx
            .tpalloc_view(self.bvname(), self.bvsizeof()?)
            .map_err(|e| UbfError::new(UbfError::BMALLOC, e.message))?;
        if !template.bvnull(field, occurrence)? {
            return Err(UbfError::new(
                UbfError::BEINVAL,
                "VIEW field has no NULL sentinel",
            ));
        }
        reset_occurrence(self, &template, field, occurrence)
    }
}

fn cstring(value: &str) -> UbfResult<CString> {
    CString::new(value)
        .map_err(|_| UbfError::new(UbfError::BEINVAL, "VIEW name/field contains NUL"))
}
#[doc(hidden)]
pub fn check_view(view: &TypedView<'_>, expected: &str) -> UbfResult<()> {
    if view.bvname() == expected {
        Ok(())
    } else {
        Err(UbfError::new(
            UbfError::BTYPERR,
            format!("expected VIEW {expected}, got {}", view.bvname()),
        ))
    }
}
pub(crate) fn serialize_view<'ctx, T: ViewSerialize>(
    ctx: &'ctx AtmiCtx,
    value: &T,
) -> UbfResult<TypedView<'ctx>> {
    let size = ctx.bvsizeof(T::VIEW_NAME)?;
    let mut view = ctx
        .tpalloc_view(T::VIEW_NAME, size)
        .map_err(|e| UbfError::new(UbfError::BMALLOC, e.message))?;
    value.view_serialize(&mut view)?;
    Ok(view)
}

fn reset_occurrence(
    view: &mut TypedView<'_>,
    template: &TypedView<'_>,
    field: &str,
    occurrence: i32,
) -> UbfResult<()> {
    let (_, max, _, dim, kind) = view.bvoccur(field)?;
    if occurrence < 0 || occurrence as usize >= max {
        return Err(UbfError::new(
            UbfError::BEINVAL,
            "VIEW occurrence is outside its declared array",
        ));
    }
    let name = cstring(view.bvname())?;
    let cname = cstring(field)?;
    let mut data = vec![0u8; dim.max(32)];
    let mut len = raw::BFLDLEN::try_from(data.len())
        .map_err(|_| UbfError::new(UbfError::BEINVAL, "VIEW dimension exceeds BFLDLEN"))?;
    #[cfg(not(feature = "ctx-send"))]
    let rc = unsafe {
        raw::CBvget(
            template.buffer().as_ptr(),
            name.as_ptr() as *mut _,
            cname.as_ptr() as *mut _,
            occurrence,
            data.as_mut_ptr().cast(),
            &mut len,
            kind as c_int,
            0,
        )
    };
    #[cfg(feature = "ctx-send")]
    let rc = unsafe {
        raw::OCBvget(
            view.buffer().ctx.c_ctx_ptr(),
            template.buffer().as_ptr(),
            name.as_ptr() as *mut _,
            cname.as_ptr() as *mut _,
            occurrence,
            data.as_mut_ptr().cast(),
            &mut len,
            kind as c_int,
            0,
        )
    };
    if rc != 0 {
        return Err(view.buffer().ctx.ubf_last_error());
    }
    #[cfg(not(feature = "ctx-send"))]
    let rc = unsafe {
        raw::CBvchg(
            view.buffer().as_ptr(),
            name.as_ptr() as *mut _,
            cname.as_ptr() as *mut _,
            occurrence,
            data.as_mut_ptr().cast(),
            len,
            kind as c_int,
        )
    };
    #[cfg(feature = "ctx-send")]
    let rc = unsafe {
        raw::OCBvchg(
            view.buffer().ctx.c_ctx_ptr(),
            view.buffer().as_ptr(),
            name.as_ptr() as *mut _,
            cname.as_ptr() as *mut _,
            occurrence,
            data.as_mut_ptr().cast(),
            len,
            kind as c_int,
        )
    };
    if rc != 0 {
        Err(view.buffer().ctx.ubf_last_error())
    } else {
        Ok(())
    }
}

fn write_integer(
    view: &mut TypedView<'_>,
    field: &str,
    occurrence: i32,
    value: i64,
) -> UbfResult<()> {
    let kind = view.bvoccur(field)?.4;
    let valid = match kind as u32 {
        raw::BFLD_SHORT => i16::try_from(value).is_ok(),
        raw::BFLD_CHAR => i8::try_from(value).is_ok(),
        raw::BFLD_INT => i32::try_from(value).is_ok(),
        raw::BFLD_LONG => std::os::raw::c_long::try_from(value).is_ok(),
        _ => true,
    };
    if !valid {
        return Err(UbfError::new(
            UbfError::BTYPERR,
            "integer does not fit the VIEW field",
        ));
    }
    view.bvchg(field, occurrence, value)
}
macro_rules! integer_field {
    ($($ty:ty),* $(,)?) => { $(
        impl ViewFieldSerialize for $ty {
            fn view_write_field(&self, view: &mut TypedView<'_>, field: &str, occurrence: i32) -> UbfResult<()> {
                let value = i64::try_from(*self).map_err(|_| UbfError::new(UbfError::BTYPERR, "integer exceeds signed XATMI long"))?;
                write_integer(view, field, occurrence, value)
            }
        }
        impl ViewFieldDeserialize for $ty {
            fn view_read_field(view: &TypedView<'_>, field: &str, occurrence: i32) -> UbfResult<Self> {
                <$ty>::try_from(view.bvget_i64(field, occurrence, 0)?)
                    .map_err(|_| UbfError::new(UbfError::BTYPERR, "VIEW integer does not fit the Rust type"))
            }
        }
    )* };
}
integer_field!(i8, u8, i16, u16, i32, u32, i64, u64, isize, usize);
macro_rules! float_field {
    ($ty:ty, $get:ident) => {
        impl ViewFieldSerialize for $ty {
            fn view_write_field(
                &self,
                view: &mut TypedView<'_>,
                field: &str,
                occurrence: i32,
            ) -> UbfResult<()> {
                view.bvchg(field, occurrence, *self)
            }
        }
        impl ViewFieldDeserialize for $ty {
            fn view_read_field(
                view: &TypedView<'_>,
                field: &str,
                occurrence: i32,
            ) -> UbfResult<Self> {
                view.$get(field, occurrence, 0)
            }
        }
    };
}
float_field!(f32, bvget_f32);
float_field!(f64, bvget_f64);
impl ViewFieldSerialize for bool {
    fn view_write_field(
        &self,
        view: &mut TypedView<'_>,
        field: &str,
        occurrence: i32,
    ) -> UbfResult<()> {
        write_integer(view, field, occurrence, i64::from(*self))
    }
}
impl ViewFieldDeserialize for bool {
    fn view_read_field(view: &TypedView<'_>, field: &str, occurrence: i32) -> UbfResult<Self> {
        match view.bvget_i64(field, occurrence, 0)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(UbfError::new(
                UbfError::BTYPERR,
                "boolean VIEW field must be 0 or 1",
            )),
        }
    }
}
impl ViewFieldSerialize for String {
    fn view_write_field(
        &self,
        view: &mut TypedView<'_>,
        field: &str,
        occurrence: i32,
    ) -> UbfResult<()> {
        view.bvchg(field, occurrence, self.as_str())
    }
}
impl ViewFieldDeserialize for String {
    fn view_read_field(view: &TypedView<'_>, field: &str, occurrence: i32) -> UbfResult<Self> {
        view.bvget_string(field, occurrence, 0)
    }
}
impl ViewFieldSerialize for UbfCarray {
    fn view_write_field(
        &self,
        view: &mut TypedView<'_>,
        field: &str,
        occurrence: i32,
    ) -> UbfResult<()> {
        view.bvchg(field, occurrence, self.0.clone())
    }
}
impl ViewFieldDeserialize for UbfCarray {
    fn view_read_field(view: &TypedView<'_>, field: &str, occurrence: i32) -> UbfResult<Self> {
        view.bvget_bytes(field, occurrence, 0).map(Self)
    }
}
impl<T: ViewFieldSerialize> ViewFieldSerialize for Option<T> {
    fn view_write_field(
        &self,
        view: &mut TypedView<'_>,
        field: &str,
        occurrence: i32,
    ) -> UbfResult<()> {
        crate::ubf_serde::require_single(T::SINGLE)?;
        match self {
            Some(value) => {
                value.view_write_field(view, field, occurrence)?;
                if view.bvnull(field, occurrence)? {
                    Err(UbfError::new(
                        UbfError::BTYPERR,
                        "Some(value) equals the compiled VIEW NULL sentinel",
                    ))
                } else {
                    Ok(())
                }
            }
            None => view.bvsetnull(field, occurrence),
        }
    }
}
impl<T: ViewFieldDeserialize> ViewFieldDeserialize for Option<T> {
    fn view_read_field(view: &TypedView<'_>, field: &str, occurrence: i32) -> UbfResult<Self> {
        crate::ubf_serde::require_single(T::SINGLE)?;
        if view.bvnull(field, occurrence)? {
            Ok(None)
        } else {
            T::view_read_field(view, field, occurrence).map(Some)
        }
    }
}
impl<T: ViewFieldSerialize> ViewFieldSerialize for Vec<T> {
    const SINGLE: bool = false;
    fn view_write_field(
        &self,
        view: &mut TypedView<'_>,
        field: &str,
        occurrence: i32,
    ) -> UbfResult<()> {
        crate::ubf_serde::require_single(T::SINGLE)?;
        let end = ubf_occurrence(occurrence, self.len())?;
        let max = view.bvoccur(field)?.1;
        if end as usize > max {
            return Err(UbfError::new(
                UbfError::BNOSPACE,
                "vector exceeds the VIEW occurrence capacity",
            ));
        }
        // Without a C counter only the full fixed array is representable.
        view.bvsetoccur(field, end)?;
        if view.bvoccur(field)?.0 != end as usize {
            return Err(UbfError::new(
                UbfError::BEINVAL,
                "variable-length VIEW vectors require a C occurrence indicator",
            ));
        }
        for (index, value) in self.iter().enumerate() {
            value.view_write_field(view, field, ubf_occurrence(occurrence, index)?)?;
        }
        if (end as usize) < max {
            let template = view
                .buffer()
                .ctx
                .tpalloc_view(view.bvname(), view.bvsizeof()?)
                .map_err(|e| UbfError::new(UbfError::BMALLOC, e.message))?;
            for occ in end as usize..max {
                reset_occurrence(view, &template, field, occ as i32)?;
            }
        }
        view.bvsetoccur(field, end)
    }
}
impl<T: ViewFieldDeserialize> ViewFieldDeserialize for Vec<T> {
    const SINGLE: bool = false;
    fn view_read_field(view: &TypedView<'_>, field: &str, occurrence: i32) -> UbfResult<Self> {
        crate::ubf_serde::require_single(T::SINGLE)?;
        ubf_occurrence(occurrence, 0)?;
        let count = view.bvoccur(field)?.0;
        (occurrence as usize..count)
            .map(|occ| T::view_read_field(view, field, occ as i32))
            .collect()
    }
}
impl<T: ViewFieldSerialize, const N: usize> ViewFieldSerialize for [T; N] {
    const SINGLE: bool = false;
    fn view_write_field(
        &self,
        view: &mut TypedView<'_>,
        field: &str,
        occurrence: i32,
    ) -> UbfResult<()> {
        crate::ubf_serde::require_single(T::SINGLE)?;
        let end = ubf_occurrence(occurrence, N)?;
        if end as usize > view.bvoccur(field)?.1 {
            return Err(UbfError::new(
                UbfError::BNOSPACE,
                "array exceeds the VIEW occurrence capacity",
            ));
        }
        for (index, value) in self.iter().enumerate() {
            value.view_write_field(view, field, ubf_occurrence(occurrence, index)?)?;
        }
        Ok(())
    }
}
impl<T: ViewFieldDeserialize, const N: usize> ViewFieldDeserialize for [T; N] {
    const SINGLE: bool = false;
    fn view_read_field(view: &TypedView<'_>, field: &str, occurrence: i32) -> UbfResult<Self> {
        crate::ubf_serde::require_single(T::SINGLE)?;
        let end = ubf_occurrence(occurrence, N)?;
        if end as usize > view.bvoccur(field)?.0 {
            return Err(UbfError::new(
                UbfError::BNOTPRES,
                "VIEW array has fewer occurrences than required",
            ));
        }
        let values = (0..N)
            .map(|i| T::view_read_field(view, field, ubf_occurrence(occurrence, i)?))
            .collect::<UbfResult<Vec<_>>>()?;
        values
            .try_into()
            .map_err(|_| UbfError::new(UbfError::BEINVAL, "invalid VIEW array extent"))
    }
}
impl<T: ViewSerialize> ViewSerialize for Box<T> {
    const VIEW_NAME: &'static str = T::VIEW_NAME;
    fn view_serialize(&self, view: &mut TypedView<'_>) -> UbfResult<()> {
        (**self).view_serialize(view)
    }
}
impl<T: ViewDeserialize> ViewDeserialize for Box<T> {
    const VIEW_NAME: &'static str = T::VIEW_NAME;
    fn view_deserialize(view: &TypedView<'_>) -> UbfResult<Self> {
        T::view_deserialize(view).map(Box::new)
    }
}

impl ViewFieldSerialize for str {
    fn view_write_field(
        &self,
        view: &mut TypedView<'_>,
        field: &str,
        occurrence: i32,
    ) -> UbfResult<()> {
        view.bvchg(field, occurrence, self)
    }
}
impl<T: ViewFieldSerialize + ?Sized> ViewFieldSerialize for &T {
    const SINGLE: bool = T::SINGLE;
    fn view_write_field(
        &self,
        view: &mut TypedView<'_>,
        field: &str,
        occurrence: i32,
    ) -> UbfResult<()> {
        (*self).view_write_field(view, field, occurrence)
    }
}

impl<T: ViewSerialize + ?Sized> ViewSerialize for &T {
    const VIEW_NAME: &'static str = T::VIEW_NAME;
    fn view_serialize(&self, view: &mut TypedView<'_>) -> UbfResult<()> {
        (**self).view_serialize(view)
    }
}
