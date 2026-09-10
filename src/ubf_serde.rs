use crate::{TypedUbf, UbfError, UbfResult, UbfValue};

std::thread_local! {
    // Scoped to synchronous mapping calls; no native-buffer borrow is retained.
    static READ_PATH: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

struct ReadScope;
impl ReadScope {
    fn enter(ubf: &TypedUbf<'_>) -> UbfResult<Self> {
        READ_PATH.with(|path| {
            let mut path = path.borrow_mut();
            let address = ubf.as_ubfh() as usize;
            if path.len() >= 128 || path.contains(&address) {
                return Err(UbfError::new(
                    UbfError::BEINVAL,
                    "cyclic or excessively deep UBF structure mapping",
                ));
            }
            path.push(address);
            Ok(Self)
        })
    }
}
impl Drop for ReadScope {
    fn drop(&mut self) {
        READ_PATH.with(|path| {
            path.borrow_mut().pop();
        });
    }
}

/// Delete every occurrence of `field_id` from `first` upward.
///
/// Occurrences are removed from the end so the indices below `first` keep their
/// positions while the list shrinks.
fn clear_from(ubf: &mut TypedUbf<'_>, field_id: i32, first: i32) -> UbfResult<()> {
    if first < 0 {
        return Err(UbfError::new(UbfError::BEINVAL, "negative occurrence"));
    }
    let ctx = ubf.ctx();
    let mut total = ctx.boccur(ubf, field_id)? as i32;
    while total > first {
        total -= 1;
        ubf.bdel_owned(field_id, total)?;
    }
    Ok(())
}

/// Serialize a Rust structure into a UBF buffer.
///
/// This is the runtime layer intended for derive macros or hand-written
/// mappings. A derive can call [`UbfFieldSerialize::ubf_write_field`] for each
/// annotated field.
pub trait UbfSerialize {
    fn ubf_serialize<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, realloc: bool) -> UbfResult<()>;
}

/// Deserialize a Rust structure from a UBF buffer.
pub trait UbfDeserialize: Sized {
    fn ubf_deserialize<'ctx>(ubf: &TypedUbf<'ctx>) -> UbfResult<Self>;
}

/// Serialize one Rust value to one or more occurrences of a UBF field.
pub trait UbfFieldSerialize {
    /// Set to false if this mapper can consume zero or multiple occurrences.
    /// Collection elements must always consume exactly one occurrence.
    const SINGLE: bool = true;
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()>;
}

/// Deserialize one Rust value from one or more occurrences of a UBF field.
pub trait UbfFieldDeserialize: Sized {
    /// Must agree with the corresponding serializer's occurrence width.
    const SINGLE: bool = true;
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self>;
}

/// Explicit CARRAY wrapper.
///
/// Plain `Vec<T>` is reserved for repeated field occurrences. Use this wrapper
/// when the UBF field itself is a `carray`/byte blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UbfCarray(pub Vec<u8>);

/// Ad-hoc embedded UBF wrapper.
///
/// Use this for embedded UBF fields whose contents are intentionally not mapped
/// to a Rust sub-structure.
#[derive(Debug)]
pub struct UbfAdhoc<'ctx>(pub TypedUbf<'ctx>);

impl TypedUbf<'_> {
    /// Write mapped fields. On error, earlier fields may already be updated.
    pub fn ubf_write<T: UbfSerialize>(&mut self, value: &T, realloc: bool) -> UbfResult<()> {
        value.ubf_serialize(self, realloc)
    }

    /// Read owned Rust values without extracting native pointer targets.
    pub fn ubf_read<T: UbfDeserialize>(&self) -> UbfResult<T> {
        T::ubf_deserialize(self)
    }
}

fn write_integer(
    ubf: &mut TypedUbf<'_>,
    field: i32,
    occurrence: i32,
    value: i64,
    realloc: bool,
) -> UbfResult<()> {
    ubf_occurrence(occurrence, 0)?;
    let fits = match ubf.ctx().bfldtype(field)? {
        crate::UbfFieldType::Short => i16::try_from(value).is_ok(),
        crate::UbfFieldType::Char => i8::try_from(value).is_ok(),
        crate::UbfFieldType::Long => std::os::raw::c_long::try_from(value).is_ok(),
        _ => true,
    };
    if !fits {
        return Err(UbfError::new(
            UbfError::BTYPERR,
            "integer does not fit the UBF field",
        ));
    }
    ubf.bchg(field, occurrence, UbfValue::Long(value), realloc)
}
macro_rules! integer_field {
    ($($ty:ty),* $(,)?) => { $(
        impl UbfFieldSerialize for $ty {
            fn ubf_write_field<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, field: i32,
                occurrence: i32, realloc: bool) -> UbfResult<()> {
                let value = i64::try_from(*self)
                    .map_err(|_| UbfError::new(UbfError::BTYPERR, "integer exceeds signed XATMI long"))?;
                write_integer(ubf, field, occurrence, value, realloc)
            }
        }
        impl UbfFieldDeserialize for $ty {
            fn ubf_read_field<'ctx>(ubf: &TypedUbf<'ctx>, field: i32, occurrence: i32) -> UbfResult<Self> {
                ubf_occurrence(occurrence, 0)?;
                <$ty>::try_from(ubf.bget_long(field, occurrence)?)
                    .map_err(|_| UbfError::new(UbfError::BTYPERR, "UBF integer does not fit the Rust type"))
            }
        }
    )* };
}
integer_field!(i8, u8, i16, u16, i32, u32, i64, u64, isize, usize);
macro_rules! float_field {
    ($ty:ty, $getter:ident) => {
        impl UbfFieldSerialize for $ty {
            fn ubf_write_field<'ctx>(
                &self,
                ubf: &mut TypedUbf<'ctx>,
                field: i32,
                occurrence: i32,
                realloc: bool,
            ) -> UbfResult<()> {
                ubf.bchg(field, occurrence, *self, realloc)
            }
        }
        impl UbfFieldDeserialize for $ty {
            fn ubf_read_field<'ctx>(
                ubf: &TypedUbf<'ctx>,
                field: i32,
                occurrence: i32,
            ) -> UbfResult<Self> {
                ubf.$getter(field, occurrence)
            }
        }
    };
}
float_field!(f32, bget_float);
float_field!(f64, bget_double);
impl UbfFieldSerialize for bool {
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        write_integer(ubf, field, occurrence, i64::from(*self), realloc)
    }
}
impl UbfFieldDeserialize for bool {
    fn ubf_read_field<'ctx>(ubf: &TypedUbf<'ctx>, field: i32, occurrence: i32) -> UbfResult<Self> {
        match ubf.bget_long(field, occurrence)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(UbfError::new(
                UbfError::BTYPERR,
                "boolean UBF field must be 0 or 1",
            )),
        }
    }
}

impl UbfFieldSerialize for String {
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        ubf.bchg(field_id, occurrence, self.as_str(), realloc)
    }
}

impl UbfFieldSerialize for str {
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        ubf.bchg(field_id, occurrence, self, realloc)
    }
}

impl UbfFieldDeserialize for String {
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        ubf.bget_string(field_id, occurrence)
    }
}

impl UbfFieldSerialize for UbfCarray {
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        ubf.bchg(
            field_id,
            occurrence,
            UbfValue::Carray(self.0.clone()),
            realloc,
        )
    }
}

impl UbfFieldDeserialize for UbfCarray {
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        ubf.bget_bytes(field_id, occurrence).map(UbfCarray)
    }
}

impl<'value, T> UbfFieldSerialize for &'value T
where
    T: UbfFieldSerialize + ?Sized,
{
    const SINGLE: bool = T::SINGLE;
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        (*self).ubf_write_field(ubf, field_id, occurrence, realloc)
    }
}

impl<T> UbfFieldSerialize for Option<T>
where
    T: UbfFieldSerialize,
{
    const SINGLE: bool = false;
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        ubf_occurrence(occurrence, 0)?;
        require_single(T::SINGLE)?;
        match self {
            Some(value) => value.ubf_write_field(ubf, field_id, occurrence, realloc),
            None => {
                // Serializing is a replace, not a merge. Leaving the previous
                // value in place would make a `None` round-trip back as `Some`.
                clear_from(ubf, field_id, occurrence)
            }
        }
    }
}

impl<T> UbfFieldDeserialize for Option<T>
where
    T: UbfFieldDeserialize,
{
    const SINGLE: bool = false;
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        require_single(T::SINGLE)?;
        if ubf_mapping_present(ubf, field_id, occurrence)? {
            T::ubf_read_field(ubf, field_id, occurrence).map(Some)
        } else {
            Ok(None)
        }
    }
}

impl<T> UbfFieldSerialize for Vec<T>
where
    T: UbfFieldSerialize,
{
    const SINGLE: bool = false;
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        require_single(T::SINGLE)?;
        let end = ubf_occurrence(occurrence, self.len())?;
        for (idx, value) in self.iter().enumerate() {
            value.ubf_write_field(ubf, field_id, ubf_occurrence(occurrence, idx)?, realloc)?;
        }
        clear_from(ubf, field_id, end)
    }
}

impl<T> UbfFieldDeserialize for Vec<T>
where
    T: UbfFieldDeserialize,
{
    const SINGLE: bool = false;
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        require_single(T::SINGLE)?;
        ubf_occurrence(occurrence, 0)?;
        let total = ubf.ctx().boccur(ubf, field_id)? as i32;
        let mut values = Vec::new();
        for occ in occurrence..total {
            values.push(T::ubf_read_field(ubf, field_id, occ)?);
        }
        Ok(values)
    }
}

impl UbfFieldSerialize for UbfAdhoc<'_> {
    fn ubf_write_field<'buf>(
        &self,
        ubf: &mut TypedUbf<'buf>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        // A fresh single-owner deep copy, so the owned-embed transfer is safe
        // even when the ad-hoc buffer carries pointer fields.
        let copy = self.0.clone_into(ubf.ctx())?;
        ubf.put_owned_ubf(field_id, occurrence, copy, realloc)
    }
}

/// Write an owned ad-hoc embedded UBF field.
pub fn ubf_write_adhoc<'ctx>(
    ubf: &mut TypedUbf<'ctx>,
    field_id: i32,
    occurrence: i32,
    value: UbfAdhoc<'ctx>,
    realloc: bool,
) -> UbfResult<()> {
    ubf.bchg(field_id, occurrence, value.0, realloc)
}

/// Write a nested Rust structure as an embedded UBF field.
pub fn ubf_write_nested<T: UbfSerialize>(
    ubf: &mut TypedUbf<'_>,
    field_id: i32,
    occurrence: i32,
    value: &T,
    initial_size: usize,
    realloc: bool,
) -> UbfResult<()> {
    let ctx = ubf.ctx();
    let mut nested = ctx.tpalloc_ubf(initial_size).map_err(|e| {
        UbfError::new(
            UbfError::BMALLOC,
            format!("failed to allocate nested UBF: {}", e.message),
        )
    })?;
    value.ubf_serialize(&mut nested, realloc)?;
    ubf.bchg(field_id, occurrence, nested, realloc)
}

/// Read a nested Rust structure from an embedded UBF field.
pub fn ubf_read_nested<T: UbfDeserialize>(
    ubf: &TypedUbf<'_>,
    field_id: i32,
    occurrence: i32,
) -> UbfResult<T> {
    let nested = ubf.bget_ubf(field_id, occurrence)?;
    let nested = unsafe { TypedUbf::borrowed_from_raw(nested.ctx, nested.ptr as *mut _) };
    let _scope = ReadScope::enter(&nested)?;
    T::ubf_deserialize(&nested)
}

/// Read an embedded UBF with a caller supplied mapper.
///
/// This is the ad-hoc escape hatch for sub-structures that need to inspect a
/// nested UBF without declaring a fixed Rust schema for it.
pub fn ubf_read_adhoc<R>(
    ubf: &TypedUbf<'_>,
    field_id: i32,
    occurrence: i32,
    f: impl FnOnce(&TypedUbf<'_>) -> UbfResult<R>,
) -> UbfResult<R> {
    let nested = ubf.bget_ubf(field_id, occurrence)?;
    let nested = unsafe { TypedUbf::borrowed_from_raw(nested.ctx, nested.ptr as *mut _) };
    let _scope = ReadScope::enter(&nested)?;
    f(&nested)
}

/// Add an occurrence offset without truncating or overflowing XATMI's index.
#[doc(hidden)]
pub fn ubf_occurrence(first: i32, index: usize) -> UbfResult<i32> {
    if first < 0 {
        return Err(UbfError::new(UbfError::BEINVAL, "negative occurrence"));
    }
    i32::try_from(index)
        .ok()
        .and_then(|index| first.checked_add(index))
        .ok_or_else(|| UbfError::new(UbfError::BEINVAL, "occurrence index overflow"))
}

/// Storage strategy for a mapped complex field.
pub trait UbfMapping {
    const FIELD_TYPE: crate::UbfFieldType;
}
/// Store a structure inline in a `BFLD_UBF` occurrence.
pub struct EmbeddedUbf;
/// Store a structure in an owned UBF allocation referenced by `BFLD_PTR`.
pub struct PointerUbf;
/// Store a structure inline in a `BFLD_VIEW` occurrence.
pub struct EmbeddedView;
/// Store a structure in an owned VIEW allocation referenced by `BFLD_PTR`.
pub struct PointerView;
impl UbfMapping for EmbeddedUbf {
    const FIELD_TYPE: crate::UbfFieldType = crate::UbfFieldType::Ubf;
}
impl UbfMapping for PointerUbf {
    const FIELD_TYPE: crate::UbfFieldType = crate::UbfFieldType::Ptr;
}
impl UbfMapping for EmbeddedView {
    const FIELD_TYPE: crate::UbfFieldType = crate::UbfFieldType::View;
}
impl UbfMapping for PointerView {
    const FIELD_TYPE: crate::UbfFieldType = crate::UbfFieldType::Ptr;
}

/// Write a structure, optional structure, vector, or fixed array using strategy `M`.
pub trait UbfMappedSerialize<M: UbfMapping> {
    /// Whether a value always occupies exactly one occurrence.
    const SINGLE: bool = true;
    /// Marks a nullable wrapper to reject ambiguous nested optional values.
    const OPTIONAL: bool = false;
    fn ubf_write_mapped(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        initial_size: usize,
        realloc: bool,
    ) -> UbfResult<()>;
    /// Write an element inside a collection, preserving its physical position.
    fn ubf_write_occurrence(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        initial_size: usize,
        realloc: bool,
    ) -> UbfResult<()> {
        self.ubf_write_mapped(ubf, field, occurrence, initial_size, realloc)
    }
}
/// Read owned Rust values using the same storage strategy as serialization.
pub trait UbfMappedDeserialize<M: UbfMapping>: Sized {
    /// Must agree with the corresponding mapped serializer.
    const SINGLE: bool = true;
    /// Must agree with the corresponding mapped serializer.
    const OPTIONAL: bool = false;
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self>;
}

fn mapped_field<M: UbfMapping>(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<()> {
    ubf_occurrence(occurrence, 0)?;
    ubf.require_field_type(field, M::FIELD_TYPE, "structure mapping")
}

impl<T: UbfSerialize> UbfMappedSerialize<EmbeddedUbf> for T {
    fn ubf_write_mapped(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        size: usize,
        realloc: bool,
    ) -> UbfResult<()> {
        mapped_field::<EmbeddedUbf>(ubf, field, occurrence)?;
        ubf_write_nested(ubf, field, occurrence, self, size, realloc)
    }
}
impl<T: UbfDeserialize> UbfMappedDeserialize<EmbeddedUbf> for T {
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        mapped_field::<EmbeddedUbf>(ubf, field, occurrence)?;
        ubf_read_nested(ubf, field, occurrence)
    }
}

/// Serialize one structure into a newly owned UBF pointer target.
pub fn ubf_write_ptr<T: UbfSerialize>(
    ubf: &mut TypedUbf<'_>,
    field: i32,
    occurrence: i32,
    value: &T,
    initial_size: usize,
    realloc: bool,
) -> UbfResult<()> {
    mapped_field::<PointerUbf>(ubf, field, occurrence)?;
    let mut target = ubf
        .ctx()
        .tpalloc_ubf(initial_size)
        .map_err(|e| UbfError::new(UbfError::BMALLOC, e.message))?;
    value.ubf_serialize(&mut target, realloc)?;
    ubf.bchg(field, occurrence, target.into_inner(), realloc)
}

/// Deserialize through a UBF pointer without extracting or taking its ownership.
pub fn ubf_read_ptr<T: UbfDeserialize>(
    ubf: &TypedUbf<'_>,
    field: i32,
    occurrence: i32,
) -> UbfResult<T> {
    mapped_field::<PointerUbf>(ubf, field, occurrence)?;
    let target = ubf.bget_ptr_ubf(field, occurrence)?;
    let target = unsafe { TypedUbf::borrowed_from_raw(target.ctx, target.ptr.cast()) };
    let _scope = ReadScope::enter(&target)?;
    T::ubf_deserialize(&target)
}
impl<T: UbfSerialize> UbfMappedSerialize<PointerUbf> for T {
    fn ubf_write_mapped(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        size: usize,
        realloc: bool,
    ) -> UbfResult<()> {
        ubf_write_ptr(ubf, field, occurrence, self, size, realloc)
    }
}
impl<T: UbfDeserialize> UbfMappedDeserialize<PointerUbf> for T {
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        ubf_read_ptr(ubf, field, occurrence)
    }
}

impl<T: crate::ViewSerialize> UbfMappedSerialize<EmbeddedView> for T {
    fn ubf_write_mapped(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        _size: usize,
        realloc: bool,
    ) -> UbfResult<()> {
        mapped_field::<EmbeddedView>(ubf, field, occurrence)?;
        let view = crate::view_serde::serialize_view(ubf.ctx(), self)?;
        ubf.bchg_view(field, occurrence, &view, realloc)
    }
}
impl<T: crate::ViewDeserialize> UbfMappedDeserialize<EmbeddedView> for T {
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        mapped_field::<EmbeddedView>(ubf, field, occurrence)?;
        ubf.bget_view(field, occurrence)?.view_read()
    }
}
impl<T: crate::ViewSerialize> UbfMappedSerialize<PointerView> for T {
    fn ubf_write_mapped(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        _size: usize,
        realloc: bool,
    ) -> UbfResult<()> {
        mapped_field::<PointerView>(ubf, field, occurrence)?;
        let view = crate::view_serde::serialize_view(ubf.ctx(), self)?;
        ubf.bchg(field, occurrence, view.into_inner(), realloc)
    }
}
impl<T: crate::ViewDeserialize> UbfMappedDeserialize<PointerView> for T {
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        mapped_field::<PointerView>(ubf, field, occurrence)?;
        let info = ubf
            .bget_ptr(field, occurrence)?
            .tptypes()
            .map_err(|e| UbfError::new(UbfError::BTYPERR, e.message))?;
        if info.type_name != "VIEW" || info.subtype != T::VIEW_NAME {
            return Err(UbfError::new(
                UbfError::BTYPERR,
                "pointer target does not match the mapped VIEW",
            ));
        }
        crate::TypedView::from_typed(T::VIEW_NAME, ubf.clone_pointer_target(field, occurrence)?)?
            .view_read()
    }
}

pub(crate) fn require_single(single: bool) -> UbfResult<()> {
    if single {
        Ok(())
    } else {
        Err(UbfError::new(UbfError::BEINVAL,
            "a collection element must occupy one occurrence; wrap nested collections or optional elements in a mapped struct"))
    }
}

impl<T: UbfMappedSerialize<M>, M: UbfMapping> UbfMappedSerialize<M> for Option<T> {
    const SINGLE: bool = matches!(
        M::FIELD_TYPE,
        crate::UbfFieldType::Ptr | crate::UbfFieldType::View
    );
    const OPTIONAL: bool = true;
    fn ubf_write_mapped(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        size: usize,
        realloc: bool,
    ) -> UbfResult<()> {
        mapped_field::<M>(ubf, field, occurrence)?;
        require_single(T::SINGLE && !T::OPTIONAL)?;
        match self {
            Some(value) => value.ubf_write_mapped(ubf, field, occurrence, size, realloc),
            None => clear_from(ubf, field, occurrence),
        }
    }
    fn ubf_write_occurrence(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        size: usize,
        realloc: bool,
    ) -> UbfResult<()> {
        mapped_field::<M>(ubf, field, occurrence)?;
        require_single(Self::SINGLE && T::SINGLE && !T::OPTIONAL)?;
        match self {
            Some(value) => value.ubf_write_occurrence(ubf, field, occurrence, size, realloc),
            None => ubf.put_null_complex(field, occurrence, realloc),
        }
    }
}
impl<T: UbfMappedDeserialize<M>, M: UbfMapping> UbfMappedDeserialize<M> for Option<T> {
    const SINGLE: bool = matches!(
        M::FIELD_TYPE,
        crate::UbfFieldType::Ptr | crate::UbfFieldType::View
    );
    const OPTIONAL: bool = true;
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        mapped_field::<M>(ubf, field, occurrence)?;
        require_single(T::SINGLE && !T::OPTIONAL)?;
        if !ubf.ctx().bpres(ubf, field, occurrence) || ubf.null_complex(field, occurrence)? {
            Ok(None)
        } else {
            T::ubf_read_mapped(ubf, field, occurrence).map(Some)
        }
    }
}
impl<T: UbfMappedSerialize<M>, M: UbfMapping> UbfMappedSerialize<M> for Vec<T> {
    const SINGLE: bool = false;
    fn ubf_write_mapped(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        size: usize,
        realloc: bool,
    ) -> UbfResult<()> {
        mapped_field::<M>(ubf, field, occurrence)?;
        require_single(T::SINGLE)?;
        let end = ubf_occurrence(occurrence, self.len())?;
        for (index, value) in self.iter().enumerate() {
            value.ubf_write_occurrence(
                ubf,
                field,
                ubf_occurrence(occurrence, index)?,
                size,
                realloc,
            )?;
        }
        clear_from(ubf, field, end)
    }
}
impl<T: UbfMappedDeserialize<M>, M: UbfMapping> UbfMappedDeserialize<M> for Vec<T> {
    const SINGLE: bool = false;
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        mapped_field::<M>(ubf, field, occurrence)?;
        require_single(T::SINGLE)?;
        let count = ubf.ctx().boccur(ubf, field)?;
        (occurrence as usize..count)
            .map(|occ| T::ubf_read_mapped(ubf, field, occ as i32))
            .collect()
    }
}
impl<T: UbfMappedSerialize<M>, M: UbfMapping, const N: usize> UbfMappedSerialize<M> for [T; N] {
    const SINGLE: bool = false;
    fn ubf_write_mapped(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        size: usize,
        realloc: bool,
    ) -> UbfResult<()> {
        mapped_field::<M>(ubf, field, occurrence)?;
        require_single(T::SINGLE)?;
        ubf_occurrence(occurrence, N)?;
        for (index, value) in self.iter().enumerate() {
            value.ubf_write_occurrence(
                ubf,
                field,
                ubf_occurrence(occurrence, index)?,
                size,
                realloc,
            )?;
        }
        Ok(())
    }
}
impl<T: UbfMappedDeserialize<M>, M: UbfMapping, const N: usize> UbfMappedDeserialize<M> for [T; N] {
    const SINGLE: bool = false;
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        mapped_field::<M>(ubf, field, occurrence)?;
        require_single(T::SINGLE)?;
        ubf_occurrence(occurrence, N)?;
        let values = (0..N)
            .map(|i| T::ubf_read_mapped(ubf, field, ubf_occurrence(occurrence, i)?))
            .collect::<UbfResult<Vec<_>>>()?;
        values
            .try_into()
            .map_err(|_| UbfError::new(UbfError::BEINVAL, "invalid array extent"))
    }
}

impl<T: UbfSerialize> UbfSerialize for Box<T> {
    fn ubf_serialize<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, realloc: bool) -> UbfResult<()> {
        (**self).ubf_serialize(ubf, realloc)
    }
}
impl<T: UbfDeserialize> UbfDeserialize for Box<T> {
    fn ubf_deserialize<'ctx>(ubf: &TypedUbf<'ctx>) -> UbfResult<Self> {
        T::ubf_deserialize(ubf).map(Box::new)
    }
}

impl<T: UbfFieldSerialize, const N: usize> UbfFieldSerialize for [T; N] {
    const SINGLE: bool = false;
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        require_single(T::SINGLE)?;
        ubf_occurrence(occurrence, N)?;
        for (index, value) in self.iter().enumerate() {
            value.ubf_write_field(ubf, field, ubf_occurrence(occurrence, index)?, realloc)?;
        }
        Ok(())
    }
}
impl<T: UbfFieldDeserialize, const N: usize> UbfFieldDeserialize for [T; N] {
    const SINGLE: bool = false;
    fn ubf_read_field<'ctx>(ubf: &TypedUbf<'ctx>, field: i32, occurrence: i32) -> UbfResult<Self> {
        require_single(T::SINGLE)?;
        ubf_occurrence(occurrence, N)?;
        let values = (0..N)
            .map(|i| T::ubf_read_field(ubf, field, ubf_occurrence(occurrence, i)?))
            .collect::<UbfResult<Vec<_>>>()?;
        values
            .try_into()
            .map_err(|_| UbfError::new(UbfError::BEINVAL, "invalid array extent"))
    }
}

/// Presence check used for `#[ubf(default)]`; malformed child contents are not
/// mistaken for a missing parent occurrence.
#[doc(hidden)]
pub fn ubf_mapping_present(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<bool> {
    ubf_occurrence(occurrence, 0)?;
    Ok((occurrence as usize) < ubf.ctx().boccur(ubf, field)?)
}

impl<T: UbfFieldSerialize + ?Sized> UbfFieldSerialize for Box<T> {
    const SINGLE: bool = T::SINGLE;
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        (**self).ubf_write_field(ubf, field, occurrence, realloc)
    }
}
impl<T: UbfFieldDeserialize> UbfFieldDeserialize for Box<T> {
    const SINGLE: bool = T::SINGLE;
    fn ubf_read_field<'ctx>(ubf: &TypedUbf<'ctx>, field: i32, occurrence: i32) -> UbfResult<Self> {
        T::ubf_read_field(ubf, field, occurrence).map(Box::new)
    }
}

impl<T: UbfSerialize + ?Sized> UbfSerialize for &T {
    fn ubf_serialize<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, realloc: bool) -> UbfResult<()> {
        (**self).ubf_serialize(ubf, realloc)
    }
}

// -------------------------------------------------------------------------
// Flat "group" mapping: a structure whose fields occupy one occurrence column
// of the *parent* buffer, and a flat `Vec`/array that walks the base
// occurrence across that shared field group. No BFLD_UBF/BFLD_PTR is used.
// -------------------------------------------------------------------------

/// A structure written into one occurrence column of the parent buffer.
///
/// `ubf_write_at` writes every mapped field at `base` (plus the field's own
/// occurrence offset). A flat `Vec<Self>` advances `base` per element.
pub trait UbfGroupSerialize {
    /// Validate the row layout and all member offsets before modifying a buffer.
    fn ubf_validate_group(base: i32) -> UbfResult<()> {
        ubf_occurrence(base, 0).map(|_| ())
    }
    fn ubf_write_at(&self, ubf: &mut TypedUbf<'_>, base: i32, realloc: bool) -> UbfResult<()>;
    /// Delete each member's suffix at `from` plus that member's offset.
    fn ubf_clear_from(ubf: &mut TypedUbf<'_>, from: i32) -> UbfResult<()>;
}
/// A structure read back from one occurrence column of the parent buffer.
pub trait UbfGroupDeserialize: Sized {
    /// Field whose occurrence count bounds a flat vector of this structure.
    const ANCHOR: i32;
    /// Occurrence offset of the anchor within each row.
    const ANCHOR_OFFSET: i32 = 0;
    fn ubf_validate_group(base: i32) -> UbfResult<()> {
        ubf_group_field_check(true, base, Self::ANCHOR_OFFSET).map(|_| ())
    }
    fn ubf_read_at(ubf: &TypedUbf<'_>, base: i32) -> UbfResult<Self>;
}

/// Field-position group mapping: a single structure, a `Vec`, or a fixed array.
pub trait UbfGroupFieldSerialize {
    fn ubf_group_write_field(
        &self,
        ubf: &mut TypedUbf<'_>,
        base: i32,
        realloc: bool,
    ) -> UbfResult<()>;
}
/// Read counterpart of [`UbfGroupFieldSerialize`].
pub trait UbfGroupFieldDeserialize: Sized {
    fn ubf_group_read_field(ubf: &TypedUbf<'_>, base: i32) -> UbfResult<Self>;
}

// A single struct wrapper is emitted by the derive (concrete `impl ... for T`),
// so these Vec/array blankets cannot overlap it.
impl<T: UbfGroupSerialize> UbfGroupFieldSerialize for Vec<T> {
    fn ubf_group_write_field(
        &self,
        ubf: &mut TypedUbf<'_>,
        base: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        let end = ubf_occurrence(base, self.len())?;
        T::ubf_validate_group(base)?;
        T::ubf_validate_group(end)?;
        for (index, item) in self.iter().enumerate() {
            item.ubf_write_at(ubf, ubf_occurrence(base, index)?, realloc)?;
        }
        T::ubf_clear_from(ubf, end)
    }
}
impl<T: UbfGroupDeserialize> UbfGroupFieldDeserialize for Vec<T> {
    fn ubf_group_read_field(ubf: &TypedUbf<'_>, base: i32) -> UbfResult<Self> {
        T::ubf_validate_group(base)?;
        ubf_group_field_check(true, base, T::ANCHOR_OFFSET)?;
        let count = ubf
            .ctx()
            .boccur(ubf, T::ANCHOR)?
            .saturating_sub(T::ANCHOR_OFFSET as usize);
        if count > base as usize {
            T::ubf_validate_group((count - 1) as i32)?;
        }
        (base as usize..count)
            .map(|occ| T::ubf_read_at(ubf, occ as i32))
            .collect()
    }
}
impl<T: UbfGroupSerialize, const N: usize> UbfGroupFieldSerialize for [T; N] {
    fn ubf_group_write_field(
        &self,
        ubf: &mut TypedUbf<'_>,
        base: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        T::ubf_validate_group(base)?;
        T::ubf_validate_group(ubf_occurrence(base, N.saturating_sub(1))?)?;
        for (index, item) in self.iter().enumerate() {
            item.ubf_write_at(ubf, ubf_occurrence(base, index)?, realloc)?;
        }
        Ok(())
    }
}
impl<T: UbfGroupDeserialize, const N: usize> UbfGroupFieldDeserialize for [T; N] {
    fn ubf_group_read_field(ubf: &TypedUbf<'_>, base: i32) -> UbfResult<Self> {
        T::ubf_validate_group(base)?;
        T::ubf_validate_group(ubf_occurrence(base, N.saturating_sub(1))?)?;
        let values = (0..N)
            .map(|i| T::ubf_read_at(ubf, ubf_occurrence(base, i)?))
            .collect::<UbfResult<Vec<_>>>()?;
        values
            .try_into()
            .map_err(|_| UbfError::new(UbfError::BEINVAL, "invalid array extent"))
    }
}

/// Delete every occurrence of one field at or after `from` (frees owned targets).
#[doc(hidden)]
pub fn ubf_group_clear(ubf: &mut TypedUbf<'_>, field_id: i32, from: i32) -> UbfResult<()> {
    clear_from(ubf, field_id, from)
}

/// Validate one group member's width and add its nonnegative occurrence offset.
#[doc(hidden)]
pub fn ubf_group_field_check(single: bool, base: i32, offset: i32) -> UbfResult<i32> {
    require_single(single)?;
    ubf_occurrence(offset, 0)?;
    ubf_occurrence(base, offset as usize)
}
