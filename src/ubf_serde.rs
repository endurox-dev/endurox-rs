//! Structure, scalar, repeated-field, and flat-group mappings for UBF buffers.
use crate::{TypedUbf, UbfError, UbfResult, UbfValue};

std::thread_local! {
    // Scoped to synchronous mapping calls; no native-buffer borrow is retained.
    static READ_PATH: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

struct ReadScope;
/// Cycle and nesting checks for recursive UBF deserialization.
impl ReadScope {
    /// Track a nested mapping read and reject cyclic or excessively deep native buffer graphs.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
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
/// Remove the completed mapping read from the current thread’s recursion path.
impl Drop for ReadScope {
    /// Remove the completed mapping read from the current thread’s recursion path.
    fn drop(&mut self) {
        READ_PATH.with(|path| {
            path.borrow_mut().pop();
        });
    }
}

/// Delete every occurrence of `field_id` from `first` upward.
///
/// # Arguments
///
/// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
/// - `field_id`: Typed UBF identifier selecting the mapped field.
/// - `first`: Nonnegative starting occurrence.
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
    /// Write this structure’s mapped fields into a UBF; an error may leave earlier fields updated.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
    fn ubf_serialize<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, realloc: bool) -> UbfResult<()>;
}

/// Deserialize a Rust structure from a UBF buffer.
pub trait UbfDeserialize: Sized {
    /// Read this structure from a UBF without extracting native pointer targets.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    fn ubf_deserialize<'ctx>(ubf: &TypedUbf<'ctx>) -> UbfResult<Self>;
}

/// Serialize one Rust value to one or more occurrences of a UBF field.
pub trait UbfFieldSerialize {
    /// Set to false if this mapper can consume zero or multiple occurrences.
    /// Collection elements must always consume exactly one occurrence.
    const SINGLE: bool = true;
    /// Write this value to its mapped field occurrences; errors may leave partial updates.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
    /// Read this value from its mapped field occurrences into owned Rust data.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
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

/// Convenience methods for writing and reading mapped Rust structures.
impl TypedUbf<'_> {
    /// Write mapped fields. On error, earlier fields may already be updated.
    ///
    /// # Arguments
    ///
    /// - `value`: Rust value whose mapped fields are written to the destination.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
    pub fn ubf_write<T: UbfSerialize>(&mut self, value: &T, realloc: bool) -> UbfResult<()> {
        value.ubf_serialize(self, realloc)
    }

    /// Read owned Rust values without extracting native pointer targets.
    pub fn ubf_read<T: UbfDeserialize>(&self) -> UbfResult<T> {
        T::ubf_deserialize(self)
    }
}

/// Check native integer bounds before writing one numeric occurrence.
///
/// # Arguments
///
/// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
/// - `field`: Typed UBF identifier selecting the mapped field.
/// - `occurrence`: Zero-based starting occurrence; collections occupy successive occurrences
///   from here.
/// - `value`: Signed integer checked against the destination field’s native width.
/// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Implement integer field mappings with checked native and Rust ranges.
macro_rules! integer_field {
    ($($ty:ty),* $(,)?) => { $(
        /// Write one integer after checking both Rust-to-native and native field-width bounds.
        impl UbfFieldSerialize for $ty {
            /// Write one integer after checking both Rust-to-native and native field-width bounds.
            ///
            /// # Arguments
            ///
            /// - `ubf`: Destination UBF; serialization can leave partial updates if a later
            ///   write fails.
            /// - `field`: Typed UBF identifier selecting the mapped field.
            /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
            ///   occurrences from here.
            /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
            fn ubf_write_field<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, field: i32,
                occurrence: i32, realloc: bool) -> UbfResult<()> {
                let value = i64::try_from(*self)
                    .map_err(|_| UbfError::new(UbfError::BTYPERR, "integer exceeds signed XATMI long"))?;
                write_integer(ubf, field, occurrence, value, realloc)
            }
        }
        /// Read one integer with native conversion and reject values outside the Rust type’s range.
        impl UbfFieldDeserialize for $ty {
            /// Read one integer with native conversion and reject values outside the Rust
            /// type’s range.
            ///
            /// # Arguments
            ///
            /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
            /// - `field`: Typed UBF identifier selecting the mapped field.
            /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
            ///   occurrences from here.
            fn ubf_read_field<'ctx>(ubf: &TypedUbf<'ctx>, field: i32, occurrence: i32) -> UbfResult<Self> {
                ubf_occurrence(occurrence, 0)?;
                <$ty>::try_from(ubf.bget_long(field, occurrence)?)
                    .map_err(|_| UbfError::new(UbfError::BTYPERR, "UBF integer does not fit the Rust type"))
            }
        }
    )* };
}
integer_field!(i8, u8, i16, u16, i32, u32, i64, u64, isize, usize);
/// Implement floating-point field mappings using the selected native getter.
macro_rules! float_field {
    ($ty:ty, $getter:ident) => {
        /// Write one floating-point occurrence using native type conversion.
        impl UbfFieldSerialize for $ty {
            /// Write one floating-point occurrence using native type conversion.
            ///
            /// # Arguments
            ///
            /// - `ubf`: Destination UBF; serialization can leave partial updates if a later
            ///   write fails.
            /// - `field`: Typed UBF identifier selecting the mapped field.
            /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
            ///   occurrences from here.
            /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
        /// Read one floating-point occurrence using native conversion.
        impl UbfFieldDeserialize for $ty {
            /// Read one floating-point occurrence using native conversion.
            ///
            /// # Arguments
            ///
            /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
            /// - `field`: Typed UBF identifier selecting the mapped field.
            /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
            ///   occurrences from here.
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
/// Write a boolean as integer zero or one after checking the native field width.
impl UbfFieldSerialize for bool {
    /// Write a boolean as integer zero or one after checking the native field width.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Read integer zero or one as a boolean; reject any other value.
impl UbfFieldDeserialize for bool {
    /// Read integer zero or one as a boolean; reject any other value.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
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

/// Write an owned string’s contents into one occurrence.
impl UbfFieldSerialize for String {
    /// Write an owned string’s contents into one occurrence.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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

/// Copy this string slice into one occurrence.
impl UbfFieldSerialize for str {
    /// Copy this string slice into one occurrence.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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

/// Read one occurrence into an owned string.
impl UbfFieldDeserialize for String {
    /// Read one occurrence into an owned string.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        ubf.bget_string(field_id, occurrence)
    }
}

/// Write the byte vector as one CARRAY occurrence, preserving its byte length.
impl UbfFieldSerialize for UbfCarray {
    /// Write the byte vector as one CARRAY occurrence, preserving its byte length.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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

/// Read one binary occurrence into a CARRAY wrapper.
impl UbfFieldDeserialize for UbfCarray {
    /// Read one binary occurrence into a CARRAY wrapper.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        ubf.bget_bytes(field_id, occurrence).map(UbfCarray)
    }
}

/// Delegate field serialization to the referenced value.
impl<'value, T> UbfFieldSerialize for &'value T
where
    T: UbfFieldSerialize + ?Sized,
{
    const SINGLE: bool = T::SINGLE;
    /// Delegate field serialization to the referenced value.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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

/// Write `Some` as one value; for `None`, remove this field’s suffix from the starting occurrence.
impl<T> UbfFieldSerialize for Option<T>
where
    T: UbfFieldSerialize,
{
    const SINGLE: bool = false;
    /// Write `Some` as one value; for `None`, remove this field’s suffix from the starting
    /// occurrence.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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

/// Read an optional scalar; absence becomes `None`, while malformed values remain errors.
impl<T> UbfFieldDeserialize for Option<T>
where
    T: UbfFieldDeserialize,
{
    const SINGLE: bool = false;
    /// Read an optional scalar; absence becomes `None`, while malformed values remain errors.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
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

/// Write consecutive vector elements and remove stale occurrences after the new suffix.
impl<T> UbfFieldSerialize for Vec<T>
where
    T: UbfFieldSerialize,
{
    const SINGLE: bool = false;
    /// Write consecutive vector elements and remove stale occurrences after the new suffix.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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

/// Read the field’s suffix into a vector, starting at the requested occurrence.
impl<T> UbfFieldDeserialize for Vec<T>
where
    T: UbfFieldDeserialize,
{
    const SINGLE: bool = false;
    /// Read the field’s suffix into a vector, starting at the requested occurrence.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
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

/// Deep-copy an ad-hoc UBF and embed the copy, transferring ownership of its copied targets.
impl UbfFieldSerialize for UbfAdhoc<'_> {
    /// Deep-copy an ad-hoc UBF and embed the copy, transferring ownership of its copied targets.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field_id`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
///
/// # Arguments
///
/// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
/// - `field_id`: Typed UBF identifier selecting the mapped field.
/// - `occurrence`: Zero-based starting occurrence; collections occupy successive occurrences
///   from here.
/// - `value`: Owned ad-hoc UBF to consume and embed; this path requires it to contain no
///   pointer fields.
/// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
///
/// # Arguments
///
/// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
/// - `field_id`: Typed UBF identifier selecting the mapped field.
/// - `occurrence`: Zero-based starting occurrence; collections occupy successive occurrences
///   from here.
/// - `value`: Rust value whose mapped fields are written to the destination.
/// - `initial_size`: Initial child UBF allocation size in bytes; VIEW mappings use their
///   compiled layout size.
/// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
///
/// # Arguments
///
/// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
/// - `field_id`: Typed UBF identifier selecting the mapped field.
/// - `occurrence`: Zero-based starting occurrence; collections occupy successive occurrences
///   from here.
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
/// # Arguments
///
/// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
/// - `field_id`: Typed UBF identifier selecting the mapped field.
/// - `occurrence`: Zero-based starting occurrence; collections occupy successive occurrences
///   from here.
/// - `f`: Mapper called with a read-only borrowed child; its result must not retain that child
///   borrow.
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
///
/// # Arguments
///
/// - `first`: Nonnegative starting occurrence.
/// - `index`: Unsigned offset to add using checked native occurrence arithmetic.
///
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
/// Native field kind used by the `EmbeddedUbf` storage strategy.
impl UbfMapping for EmbeddedUbf {
    const FIELD_TYPE: crate::UbfFieldType = crate::UbfFieldType::Ubf;
}
/// Native field kind used by the `PointerUbf` storage strategy.
impl UbfMapping for PointerUbf {
    const FIELD_TYPE: crate::UbfFieldType = crate::UbfFieldType::Ptr;
}
/// Native field kind used by the `EmbeddedView` storage strategy.
impl UbfMapping for EmbeddedView {
    const FIELD_TYPE: crate::UbfFieldType = crate::UbfFieldType::View;
}
/// Native field kind used by the `PointerView` storage strategy.
impl UbfMapping for PointerView {
    const FIELD_TYPE: crate::UbfFieldType = crate::UbfFieldType::Ptr;
}

/// Write a structure, optional structure, vector, or fixed array using strategy `M`.
pub trait UbfMappedSerialize<M: UbfMapping> {
    /// Whether a value always occupies exactly one occurrence.
    const SINGLE: bool = true;
    /// Marks a nullable wrapper to reject ambiguous nested optional values.
    const OPTIONAL: bool = false;
    /// Write a complex value using storage strategy `M`, which selects inline or pointer storage.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `initial_size`: Initial child UBF allocation size in bytes; VIEW mappings use their
    ///   compiled layout size.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
    fn ubf_write_mapped(
        &self,
        ubf: &mut TypedUbf<'_>,
        field: i32,
        occurrence: i32,
        initial_size: usize,
        realloc: bool,
    ) -> UbfResult<()>;
    /// Write an element inside a collection, preserving its physical position.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `initial_size`: Initial child UBF allocation size in bytes; VIEW mappings use their
    ///   compiled layout size.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
    /// Read a complex value using storage strategy `M` without extracting native ownership.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self>;
}

/// Validate the occurrence and ensure the native field kind matches the mapping strategy.
///
/// # Arguments
///
/// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
/// - `field`: Typed UBF identifier selecting the mapped field.
/// - `occurrence`: Zero-based starting occurrence; collections occupy successive occurrences
///   from here.
fn mapped_field<M: UbfMapping>(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<()> {
    ubf_occurrence(occurrence, 0)?;
    ubf.require_field_type(field, M::FIELD_TYPE, "structure mapping")
}

/// Serialize one structure into a pointer-free inline UBF occurrence.
impl<T: UbfSerialize> UbfMappedSerialize<EmbeddedUbf> for T {
    /// Serialize one structure into a pointer-free inline UBF occurrence.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `size`: Initial child UBF allocation size in bytes, forwarded through the selected
    ///   mapping.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Deserialize a structure from an inline UBF while retaining the parent’s ownership.
impl<T: UbfDeserialize> UbfMappedDeserialize<EmbeddedUbf> for T {
    /// Deserialize a structure from an inline UBF while retaining the parent’s ownership.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        mapped_field::<EmbeddedUbf>(ubf, field, occurrence)?;
        ubf_read_nested(ubf, field, occurrence)
    }
}

/// Serialize one structure into a newly owned UBF pointer target.
///
/// # Arguments
///
/// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
/// - `field`: Typed UBF identifier selecting the mapped field.
/// - `occurrence`: Zero-based starting occurrence; collections occupy successive occurrences
///   from here.
/// - `value`: Rust value whose mapped fields are written to the destination.
/// - `initial_size`: Initial child UBF allocation size in bytes; VIEW mappings use their
///   compiled layout size.
/// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
///
/// # Arguments
///
/// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
/// - `field`: Typed UBF identifier selecting the mapped field.
/// - `occurrence`: Zero-based starting occurrence; collections occupy successive occurrences
///   from here.
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
/// Serialize one structure into a new UBF allocation owned by a pointer occurrence.
impl<T: UbfSerialize> UbfMappedSerialize<PointerUbf> for T {
    /// Serialize one structure into a new UBF allocation owned by a pointer occurrence.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `size`: Initial child UBF allocation size in bytes, forwarded through the selected
    ///   mapping.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Deserialize a structure through a UBF pointer while retaining the parent’s ownership.
impl<T: UbfDeserialize> UbfMappedDeserialize<PointerUbf> for T {
    /// Deserialize a structure through a UBF pointer while retaining the parent’s ownership.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        ubf_read_ptr(ubf, field, occurrence)
    }
}

/// Serialize one structure into a VIEW and copy its layout into an inline occurrence.
impl<T: crate::ViewSerialize> UbfMappedSerialize<EmbeddedView> for T {
    /// Serialize one structure into a VIEW and copy its layout into an inline occurrence.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `_size`: Unused allocation hint; VIEW storage size comes from its compiled layout.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Copy an inline VIEW and deserialize its mapped members.
impl<T: crate::ViewDeserialize> UbfMappedDeserialize<EmbeddedView> for T {
    /// Copy an inline VIEW and deserialize its mapped members.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        mapped_field::<EmbeddedView>(ubf, field, occurrence)?;
        ubf.bget_view(field, occurrence)?.view_read()
    }
}
/// Serialize one structure into a VIEW allocation owned by a pointer occurrence.
impl<T: crate::ViewSerialize> UbfMappedSerialize<PointerView> for T {
    /// Serialize one structure into a VIEW allocation owned by a pointer occurrence.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `_size`: Unused allocation hint; VIEW storage size comes from its compiled layout.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Validate a VIEW pointer’s type and layout, then deserialize an independent copy.
impl<T: crate::ViewDeserialize> UbfMappedDeserialize<PointerView> for T {
    /// Validate a VIEW pointer’s type and layout, then deserialize an independent copy.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
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

/// Require a mapping that consumes exactly one native occurrence per element.
///
/// # Arguments
///
/// - `single`: Whether each mapped element occupies exactly one occurrence.
pub(crate) fn require_single(single: bool) -> UbfResult<()> {
    if single {
        Ok(())
    } else {
        Err(UbfError::new(UbfError::BEINVAL,
            "a collection element must occupy one occurrence; wrap nested collections or optional elements in a mapped struct"))
    }
}

/// Write `Some` using the selected strategy; for `None`, clear the field’s suffix.
impl<T: UbfMappedSerialize<M>, M: UbfMapping> UbfMappedSerialize<M> for Option<T> {
    const SINGLE: bool = matches!(
        M::FIELD_TYPE,
        crate::UbfFieldType::Ptr | crate::UbfFieldType::View
    );
    const OPTIONAL: bool = true;
    /// Write `Some` using the selected strategy; for `None`, clear the field’s suffix.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `size`: Initial child UBF allocation size in bytes, forwarded through the selected
    ///   mapping.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
    /// Write one optional collection element, using a native NULL PTR/VIEW placeholder for `None`.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `size`: Initial child UBF allocation size in bytes, forwarded through the selected
    ///   mapping.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Read an optional complex value; absent occurrences and native NULL placeholders become `None`.
impl<T: UbfMappedDeserialize<M>, M: UbfMapping> UbfMappedDeserialize<M> for Option<T> {
    const SINGLE: bool = matches!(
        M::FIELD_TYPE,
        crate::UbfFieldType::Ptr | crate::UbfFieldType::View
    );
    const OPTIONAL: bool = true;
    /// Read an optional complex value; absent occurrences and native NULL placeholders become
    /// `None`.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
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
/// Write consecutive complex values, preserve nullable positions, and trim the stale suffix.
impl<T: UbfMappedSerialize<M>, M: UbfMapping> UbfMappedSerialize<M> for Vec<T> {
    const SINGLE: bool = false;
    /// Write consecutive complex values, preserve nullable positions, and trim the stale suffix.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `size`: Initial child UBF allocation size in bytes, forwarded through the selected
    ///   mapping.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Read the complex field’s suffix into a vector, including nullable positions.
impl<T: UbfMappedDeserialize<M>, M: UbfMapping> UbfMappedDeserialize<M> for Vec<T> {
    const SINGLE: bool = false;
    /// Read the complex field’s suffix into a vector, including nullable positions.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    fn ubf_read_mapped(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<Self> {
        mapped_field::<M>(ubf, field, occurrence)?;
        require_single(T::SINGLE)?;
        let count = ubf.ctx().boccur(ubf, field)?;
        (occurrence as usize..count)
            .map(|occ| T::ubf_read_mapped(ubf, field, occ as i32))
            .collect()
    }
}
/// Write exactly `N` complex occurrences, preserving nullable positions and neighboring
/// occurrences.
impl<T: UbfMappedSerialize<M>, M: UbfMapping, const N: usize> UbfMappedSerialize<M> for [T; N] {
    const SINGLE: bool = false;
    /// Write exactly `N` complex occurrences, preserving nullable positions and neighboring
    /// occurrences.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `size`: Initial child UBF allocation size in bytes, forwarded through the selected
    ///   mapping.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Read exactly `N` complex occurrences into a fixed array, including nullable positions.
impl<T: UbfMappedDeserialize<M>, M: UbfMapping, const N: usize> UbfMappedDeserialize<M> for [T; N] {
    const SINGLE: bool = false;
    /// Read exactly `N` complex occurrences into a fixed array, including nullable positions.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
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

/// Delegate structure serialization to the boxed value.
impl<T: UbfSerialize> UbfSerialize for Box<T> {
    /// Delegate structure serialization to the boxed value.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
    fn ubf_serialize<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, realloc: bool) -> UbfResult<()> {
        (**self).ubf_serialize(ubf, realloc)
    }
}
/// Deserialize the structure and allocate it in a Rust box.
impl<T: UbfDeserialize> UbfDeserialize for Box<T> {
    /// Deserialize the structure and allocate it in a Rust box.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    fn ubf_deserialize<'ctx>(ubf: &TypedUbf<'ctx>) -> UbfResult<Self> {
        T::ubf_deserialize(ubf).map(Box::new)
    }
}

/// Write exactly `N` consecutive occurrences, preserving occurrences outside the array window.
impl<T: UbfFieldSerialize, const N: usize> UbfFieldSerialize for [T; N] {
    const SINGLE: bool = false;
    /// Write exactly `N` consecutive occurrences, preserving occurrences outside the array window.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Read exactly `N` consecutive occurrences into a fixed array.
impl<T: UbfFieldDeserialize, const N: usize> UbfFieldDeserialize for [T; N] {
    const SINGLE: bool = false;
    /// Read exactly `N` consecutive occurrences into a fixed array.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
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
///
/// # Arguments
///
/// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
/// - `field`: Typed UBF identifier selecting the mapped field.
/// - `occurrence`: Zero-based starting occurrence; collections occupy successive occurrences
///   from here.
///
#[doc(hidden)]
pub fn ubf_mapping_present(ubf: &TypedUbf<'_>, field: i32, occurrence: i32) -> UbfResult<bool> {
    ubf_occurrence(occurrence, 0)?;
    Ok((occurrence as usize) < ubf.ctx().boccur(ubf, field)?)
}

/// Delegate field serialization to the boxed value.
impl<T: UbfFieldSerialize + ?Sized> UbfFieldSerialize for Box<T> {
    const SINGLE: bool = T::SINGLE;
    /// Delegate field serialization to the boxed value.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Deserialize the field value and allocate it in a Rust box.
impl<T: UbfFieldDeserialize> UbfFieldDeserialize for Box<T> {
    const SINGLE: bool = T::SINGLE;
    /// Deserialize the field value and allocate it in a Rust box.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `field`: Typed UBF identifier selecting the mapped field.
    /// - `occurrence`: Zero-based starting occurrence; collections occupy successive
    ///   occurrences from here.
    fn ubf_read_field<'ctx>(ubf: &TypedUbf<'ctx>, field: i32, occurrence: i32) -> UbfResult<Self> {
        T::ubf_read_field(ubf, field, occurrence).map(Box::new)
    }
}

/// Delegate structure serialization to the referenced value.
impl<T: UbfSerialize + ?Sized> UbfSerialize for &T {
    /// Delegate structure serialization to the referenced value.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
    ///
    /// # Arguments
    ///
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
    fn ubf_validate_group(base: i32) -> UbfResult<()> {
        ubf_occurrence(base, 0).map(|_| ())
    }
    /// Write one group row into the parent’s fields at the base plus each member’s offset.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
    fn ubf_write_at(&self, ubf: &mut TypedUbf<'_>, base: i32, realloc: bool) -> UbfResult<()>;
    /// Delete each member's suffix at `from` plus that member's offset.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `from`: First row or occurrence to remove; group implementations add their member offsets.
    fn ubf_clear_from(ubf: &mut TypedUbf<'_>, from: i32) -> UbfResult<()>;
}
/// A structure read back from one occurrence column of the parent buffer.
pub trait UbfGroupDeserialize: Sized {
    /// Field whose occurrence count bounds a flat vector of this structure.
    const ANCHOR: i32;
    /// Occurrence offset of the anchor within each row.
    const ANCHOR_OFFSET: i32 = 0;
    /// Validate a group row’s base occurrence and member offsets before reading it.
    ///
    /// # Arguments
    ///
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
    fn ubf_validate_group(base: i32) -> UbfResult<()> {
        ubf_group_field_check(true, base, Self::ANCHOR_OFFSET).map(|_| ())
    }
    /// Read one group row from the parent’s fields at the base plus each member’s offset.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
    fn ubf_read_at(ubf: &TypedUbf<'_>, base: i32) -> UbfResult<Self>;
}

/// Field-position group mapping: a single structure, a `Vec`, or a fixed array.
pub trait UbfGroupFieldSerialize {
    /// Write one or more flat group rows into the parent’s occurrence columns.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
    fn ubf_group_write_field(
        &self,
        ubf: &mut TypedUbf<'_>,
        base: i32,
        realloc: bool,
    ) -> UbfResult<()>;
}
/// Read counterpart of [`UbfGroupFieldSerialize`].
pub trait UbfGroupFieldDeserialize: Sized {
    /// Read one or more flat group rows from the parent’s occurrence columns.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
    fn ubf_group_read_field(ubf: &TypedUbf<'_>, base: i32) -> UbfResult<Self>;
}

// A single struct wrapper is emitted by the derive (concrete `impl ... for T`),
// so these Vec/array blankets cannot overlap it.
/// Write vector rows and clear stale suffixes in every member column, respecting member offsets.
impl<T: UbfGroupSerialize> UbfGroupFieldSerialize for Vec<T> {
    /// Write vector rows and clear stale suffixes in every member column, respecting member
    /// offsets.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Read vector rows up to the anchor field’s count after subtracting its member offset.
impl<T: UbfGroupDeserialize> UbfGroupFieldDeserialize for Vec<T> {
    /// Read vector rows up to the anchor field’s count after subtracting its member offset.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
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
/// Write exactly `N` group rows without deleting neighboring rows.
impl<T: UbfGroupSerialize, const N: usize> UbfGroupFieldSerialize for [T; N] {
    /// Write exactly `N` group rows without deleting neighboring rows.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
    /// - `realloc`: Whether writes may grow UBF allocations when they run out of space.
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
/// Read exactly `N` consecutive group rows into a fixed array.
impl<T: UbfGroupDeserialize, const N: usize> UbfGroupFieldDeserialize for [T; N] {
    /// Read exactly `N` consecutive group rows into a fixed array.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Source UBF to read; its allocations remain owned by the original buffer.
    /// - `base`: Zero-based row position before adding each member’s occurrence offset.
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
///
/// # Arguments
///
/// - `ubf`: Destination UBF; serialization can leave partial updates if a later write fails.
/// - `field_id`: Typed UBF identifier selecting the mapped field.
/// - `from`: First row or occurrence to remove; group implementations add their member offsets.
///
#[doc(hidden)]
pub fn ubf_group_clear(ubf: &mut TypedUbf<'_>, field_id: i32, from: i32) -> UbfResult<()> {
    clear_from(ubf, field_id, from)
}

/// Validate one group member's width and add its nonnegative occurrence offset.
///
/// # Arguments
///
/// - `single`: Whether each mapped element occupies exactly one occurrence.
/// - `base`: Zero-based row position before adding each member’s occurrence offset.
/// - `offset`: Nonnegative occurrence offset for this member within each row.
///
#[doc(hidden)]
pub fn ubf_group_field_check(single: bool, base: i32, offset: i32) -> UbfResult<i32> {
    require_single(single)?;
    ubf_occurrence(offset, 0)?;
    ubf_occurrence(base, offset as usize)
}
