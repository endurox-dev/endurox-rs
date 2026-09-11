//! Ownership-aware operations used by the structure mappings.
use std::{
    collections::HashSet,
    ffi::{CStr, CString},
    os::raw::c_char,
    ptr,
};

use crate::{raw, AtmiCtx, TypedBuffer, TypedUbf, TypedView, UbfError, UbfFieldType, UbfResult};

/// Convert an ATMI allocation failure to UBF `BMALLOC`, preserving its diagnostic text.
///
/// # Arguments
///
/// - `error`: ATMI failure whose message is preserved in the UBF error.
fn allocation_error(error: crate::AtmiError) -> UbfError {
    UbfError::new(UbfError::BMALLOC, error.message)
}

/// Deep copying and edits that preserve ownership of nested UBF pointer targets.
impl<'ctx> TypedUbf<'ctx> {
    /// Copy this buffer and all pointer targets into independent allocations.
    /// Cyclic pointer graphs are rejected. Repeated references are copied separately.
    pub fn deep_clone(&self) -> UbfResult<TypedUbf<'ctx>> {
        self.clone_into(self.ctx())
    }

    /// Deep-copy this UBF and its pointer targets into allocations borrowing another context.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context used to allocate or release the affected buffers.
    pub(crate) fn clone_into<'new>(&self, ctx: &'new AtmiCtx) -> UbfResult<TypedUbf<'new>> {
        clone_ubf(self, ctx, &mut HashSet::new())
    }

    /// Delete an occurrence, also freeing any pointer targets it owns.
    /// Unlike the C-compatible `AtmiCtx::bdel`, this releases owned children.
    /// Shared/cyclic native pointer graphs must first be normalized with
    /// `deep_clone`, so removing an occurrence cannot invalidate another one.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    pub fn bdel_owned(&mut self, field: i32, occurrence: i32) -> UbfResult<()> {
        let roots = self.owned_field_roots(field, occurrence)?;
        if !roots.is_empty() {
            self.ensure_owned_tree()?;
        }
        self.ctx().bdel(self, field, occurrence)?;
        free_roots(self.ctx(), roots);
        Ok(())
    }

    /// Test whether an occurrence is absent or contains a native NULL PTR/VIEW placeholder.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    pub(crate) fn null_complex(&self, field: i32, occurrence: i32) -> UbfResult<bool> {
        if !self.ctx().bpres(self, field, occurrence) {
            return Ok(true);
        }
        let mut len = 0;
        let found = self.ctx().bfind_value(self, field, occurrence, &mut len);
        if found.is_null() {
            return Err(self.ctx().ubf_last_error());
        }
        match self.ctx().bfldtype(field)? {
            UbfFieldType::Ptr => {
                Ok(unsafe { ptr::read_unaligned(found.cast::<*mut c_char>()).is_null() })
            }
            UbfFieldType::View => {
                Ok(unsafe { ptr::read_unaligned(found.cast::<raw::BVIEWFLD>()).vname[0] == 0 })
            }
            _ => Ok(false),
        }
    }

    /// Store a PTR/VIEW NULL placeholder without shifting occurrences, freeing displaced targets.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    /// - `realloc`: Whether to grow the destination and retry on `BNOSPACE`.
    pub(crate) fn put_null_complex(
        &mut self,
        field: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        let roots = self.owned_field_roots(field, occurrence)?;
        if !roots.is_empty() {
            self.ensure_owned_tree()?;
        }
        match self.ctx().bfldtype(field)? {
            UbfFieldType::Ptr => {
                let mut null: *mut c_char = ptr::null_mut();
                self.put_native(
                    field,
                    occurrence,
                    (&mut null as *mut *mut c_char).cast(),
                    0,
                    realloc,
                )?;
            }
            UbfFieldType::View => {
                let mut empty: raw::BVIEWFLD = unsafe { std::mem::zeroed() };
                self.put_native(
                    field,
                    occurrence,
                    (&mut empty as *mut raw::BVIEWFLD).cast(),
                    0,
                    realloc,
                )?;
            }
            _ => {
                return Err(UbfError::new(
                    UbfError::BTYPERR,
                    "this field type has no NULL complex occurrence",
                ))
            }
        }
        free_roots(self.ctx(), roots);
        Ok(())
    }

    /// Reject shared targets, cycles, and excessive nesting before an ownership-changing edit.
    pub(crate) fn ensure_owned_tree(&self) -> UbfResult<()> {
        let mut seen = HashSet::new();
        seen.insert(self.as_ubfh() as usize);
        unique_targets(self, &mut seen, 0)
    }

    /// Collect the pointer targets owned by one occurrence, including pointers inside inline UBFs.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    pub(crate) fn owned_field_roots(
        &self,
        field: i32,
        occurrence: i32,
    ) -> UbfResult<Vec<*mut c_char>> {
        if !self.ctx().bpres(self, field, occurrence) {
            return Ok(Vec::new());
        }
        match self.ctx().bfldtype(field)? {
            UbfFieldType::Ptr => {
                let mut len = 0;
                let data = self.ctx().bfind_value(self, field, occurrence, &mut len);
                if data.is_null() {
                    return Err(self.ctx().ubf_last_error());
                }
                let target = unsafe { ptr::read_unaligned(data.cast::<*mut c_char>()) };
                Ok(if target.is_null() {
                    Vec::new()
                } else {
                    vec![target]
                })
            }
            UbfFieldType::Ubf => {
                let child = self.bget_ubf(field, occurrence)?;
                let child = unsafe { TypedUbf::borrowed_from_raw(child.ctx, child.ptr.cast()) };
                let mut roots = Vec::new();
                collect_roots(&child, &mut roots, 0)?;
                Ok(roots)
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Write a native field value with optional growth; ownership checks belong to the caller.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    /// - `value`: Valid native field representation; pointer fields take the address of a
    ///   pointer variable.
    /// - `len`: Native value length in bytes, or `0` when the field type determines its own size.
    /// - `realloc`: Whether to grow the destination and retry on `BNOSPACE`.
    pub(crate) fn put_native(
        &mut self,
        field: i32,
        occurrence: i32,
        value: *mut c_char,
        len: raw::BFLDLEN,
        realloc: bool,
    ) -> UbfResult<()> {
        loop {
            #[cfg(not(feature = "ctx-send"))]
            let rc = unsafe { raw::Bchg(self.as_ubfh(), field, occurrence, value, len) };
            #[cfg(feature = "ctx-send")]
            let rc = unsafe {
                raw::OBchg(
                    self.ctx().c_ctx_ptr(),
                    self.as_ubfh(),
                    field,
                    occurrence,
                    value,
                    len,
                )
            };
            if rc == 0 {
                return Ok(());
            }
            let error = self.ctx().ubf_last_error();
            if error.code != UbfError::BNOSPACE || !realloc {
                return Err(error);
            }
            self.grow_buffer()?;
        }
    }

    /// Embed a UBF as one `BFLD_UBF` occurrence (public mapping path).
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    /// - `child`: Owned child consumed by the inline write; a failed write leaves cleanup to
    ///   its Rust owner.
    /// - `realloc`: Whether to grow the destination and retry on `BNOSPACE`.
    ///
    /// Children that own `BFLD_PTR` targets are rejected: an inline embed
    /// byte-copies the child, so freeing the consumed shell would cascade into
    /// targets the parent now shares. Store such sub-structures behind a
    /// `BFLD_PTR` (`ptr`) mapping instead.
    pub(crate) fn put_embedded_ubf(
        &mut self,
        field: i32,
        occurrence: i32,
        child: TypedUbf<'ctx>,
        realloc: bool,
    ) -> UbfResult<()> {
        self.embed_ubf(field, occurrence, child, realloc, false)
    }

    /// Embed an owned UBF, transferring any pointer targets it owns to this
    /// parent. Restricted to internal callers (deep-clone reconstruction and
    /// `UbfAdhoc`) that supply a fresh single-owner deep copy, so the shared-
    /// ownership hazard of the public path cannot arise.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    /// - `child`: Owned child consumed by the inline write; a failed write leaves cleanup to
    ///   its Rust owner.
    /// - `realloc`: Whether to grow the destination and retry on `BNOSPACE`.
    pub(crate) fn put_owned_ubf(
        &mut self,
        field: i32,
        occurrence: i32,
        child: TypedUbf<'ctx>,
        realloc: bool,
    ) -> UbfResult<()> {
        self.embed_ubf(field, occurrence, child, realloc, true)
    }

    /// Copy a child inline, optionally transferring its targets, and release replaced child
    /// allocations.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    /// - `child`: Owned child consumed by the inline write; a failed write leaves cleanup to
    ///   its Rust owner.
    /// - `realloc`: Whether to grow the destination and retry on `BNOSPACE`.
    /// - `transfer_pointers`: Whether a fresh, uniquely owned child may transfer its pointer
    ///   targets to the parent.
    fn embed_ubf(
        &mut self,
        field: i32,
        occurrence: i32,
        child: TypedUbf<'ctx>,
        realloc: bool,
        transfer_pointers: bool,
    ) -> UbfResult<()> {
        self.require_field_type(field, UbfFieldType::Ubf, "embedded UBF write")?;
        // Bhasptr seeks straight to the cached pointer run, so the pointer-free
        // common case pays a single step.
        let owns_pointers = child.ctx().ubf_has_pointer_fields(&child)?;
        if owns_pointers && !transfer_pointers {
            return Err(UbfError::new(
                UbfError::BEINVAL,
                "cannot inline-embed a UBF containing BFLD_PTR fields; store it \
                 behind a BFLD_PTR (ptr) mapping instead",
            ));
        }
        let previous = self.owned_field_roots(field, occurrence)?;
        if !previous.is_empty() {
            self.ensure_owned_tree()?;
        }
        self.put_native(field, occurrence, child.as_ubfh().cast(), 0, realloc)?;

        if owns_pointers {
            // Bchg copied the inline bytes, including pointer addresses. Clear
            // the consumed shell before Drop so only the destination owns its
            // targets. Binit is a stateless header initializer; this valid owned
            // UBF has at least the minimum header size, so its preconditions hold.
            let capacity = child.ctx().bsizeof(&child)?;
            let capacity = raw::BFLDLEN::try_from(capacity)
                .map_err(|_| UbfError::new(UbfError::BEINVAL, "UBF allocation exceeds BFLDLEN"))?;
            let rc = unsafe { raw::Binit(child.as_ubfh(), capacity) };
            if rc != 0 {
                // Never cascade-free targets already installed in the destination.
                let _ = child.into_raw();
                free_roots(self.ctx(), previous);
                return Err(UbfError::new(
                    UbfError::BEINVAL,
                    "failed to clear transferred UBF shell",
                ));
            }
        }
        // A pointer-free child shares no targets, and a wiped child is now empty;
        // either way dropping it frees only the staging shell.
        drop(child);
        free_roots(self.ctx(), previous);
        Ok(())
    }

    /// Copy a VIEW into one `BFLD_VIEW` occurrence.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    /// - `view`: Source VIEW whose compiled layout bytes are copied; it retains its ownership.
    /// - `realloc`: Whether to grow the destination and retry on `BNOSPACE`.
    pub fn bchg_view(
        &mut self,
        field: i32,
        occurrence: i32,
        view: &TypedView<'_>,
        realloc: bool,
    ) -> UbfResult<()> {
        self.require_field_type(field, UbfFieldType::View, "VIEW write")?;
        let name = CString::new(view.bvname())
            .map_err(|_| UbfError::new(UbfError::BEINVAL, "VIEW name contains NUL"))?;
        let mut descriptor: raw::BVIEWFLD = unsafe { std::mem::zeroed() };
        if name.as_bytes_with_nul().len() > descriptor.vname.len() {
            return Err(UbfError::new(UbfError::BEINVAL, "VIEW name is too long"));
        }
        for (dst, src) in descriptor.vname.iter_mut().zip(name.as_bytes_with_nul()) {
            *dst = *src as c_char;
        }
        descriptor.data = view.buffer().as_ptr();
        self.put_native(
            field,
            occurrence,
            (&mut descriptor as *mut raw::BVIEWFLD).cast(),
            0,
            realloc,
        )
    }

    /// Read a `BFLD_VIEW` occurrence into an independent typed VIEW allocation.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    pub fn bget_view(&self, field: i32, occurrence: i32) -> UbfResult<TypedView<'ctx>> {
        self.require_field_type(field, UbfFieldType::View, "VIEW read")?;
        let mut len = 0;
        let found = self.ctx().bfind_value(self, field, occurrence, &mut len);
        if found.is_null() {
            return Err(self.ctx().ubf_last_error());
        }
        // Bfind returns a TLS descriptor. Copy it before another native call
        // can reuse that descriptor; its data still borrows this parent UBF.
        let descriptor = unsafe { ptr::read_unaligned(found.cast::<raw::BVIEWFLD>()) };
        let name_bytes: Vec<u8> = descriptor.vname.iter().map(|c| *c as u8).collect();
        let name = CStr::from_bytes_until_nul(&name_bytes)
            .map_err(|_| UbfError::new(UbfError::BBADVIEW, "unterminated VIEW name"))?
            .to_str()
            .map_err(|_| UbfError::new(UbfError::BBADVIEW, "invalid VIEW name"))?;
        if name.is_empty() {
            return Err(UbfError::new(UbfError::BNOTPRES, "empty VIEW occurrence"));
        }
        let required = self.ctx().bvsizeof(name)?;
        if descriptor.data.is_null() || len < 0 || (len as usize) < required {
            return Err(UbfError::new(
                UbfError::BBADVIEW,
                "embedded VIEW is shorter than its layout",
            ));
        }
        let view = self
            .ctx()
            .tpalloc_view(name, required)
            .map_err(allocation_error)?;
        unsafe {
            ptr::copy_nonoverlapping(descriptor.data, view.buffer().as_ptr(), required);
        }
        Ok(view)
    }

    /// Deep-copy the buffer referenced by a pointer occurrence without extracting it.
    ///
    /// # Arguments
    ///
    /// - `field`: Typed UBF identifier; complex operations require the matching UBF, PTR, or
    ///   VIEW kind.
    /// - `occurrence`: Zero-based occurrence to read, write, or remove.
    pub(crate) fn clone_pointer_target(
        &self,
        field: i32,
        occurrence: i32,
    ) -> UbfResult<TypedBuffer<'ctx>> {
        let target = self.bget_ptr(field, occurrence)?;
        clone_buffer(&target, self.ctx(), &mut HashSet::new())
    }
}

/// Collect pointer roots through inline UBFs, leaving each root’s descendants to native cleanup.
///
/// # Arguments
///
/// - `ubf`: UBF whose inline children and pointer occurrences are traversed.
/// - `roots`: Pointer roots collected so far, excluding descendants already owned by those roots.
/// - `depth`: Current nesting depth used to enforce the recursion limit.
fn collect_roots(ubf: &TypedUbf<'_>, roots: &mut Vec<*mut c_char>, depth: usize) -> UbfResult<()> {
    if depth > 128 {
        return Err(UbfError::new(
            UbfError::BEINVAL,
            "embedded UBF nesting exceeds 128 levels",
        ));
    }
    let mut fields = ubf.bnext();
    while let Some(field) = fields.next()? {
        match field.field_type {
            UbfFieldType::Ptr => {
                roots.extend(ubf.owned_field_roots(field.field_id, field.occurrence)?)
            }
            UbfFieldType::Ubf => {
                let nested = ubf.bget_ubf(field.field_id, field.occurrence)?;
                let nested = unsafe { TypedUbf::borrowed_from_raw(nested.ctx, nested.ptr.cast()) };
                collect_roots(&nested, roots, depth + 1)?;
            }
            _ => (),
        }
    }
    Ok(())
}

/// Free each non-null root once using native recursive buffer cleanup.
///
/// # Arguments
///
/// - `ctx`: Context used to allocate or release the affected buffers.
/// - `roots`: Owned roots detached from their parent; their recursive ownership trees must not
///   overlap.
pub(crate) fn free_roots(ctx: &AtmiCtx, roots: Vec<*mut c_char>) {
    let mut freed = HashSet::new();
    for root in roots {
        if !root.is_null() && freed.insert(root) {
            drop(unsafe { TypedBuffer::from_raw(ctx, root) });
        }
    }
}

/// Copy a typed allocation, recursively cloning UBF targets and using compiled sizes for VIEWs.
///
/// # Arguments
///
/// - `source`: Borrowed source allocation that remains unchanged.
/// - `ctx`: Context used to allocate or release the affected buffers.
/// - `path`: Addresses on the active recursive clone path, used to detect cycles.
fn clone_buffer<'ctx>(
    source: &TypedBuffer<'_>,
    ctx: &'ctx AtmiCtx,
    path: &mut HashSet<usize>,
) -> UbfResult<TypedBuffer<'ctx>> {
    let info = source.tptypes().map_err(allocation_error)?;
    if info.type_name == "UBF" {
        let source = unsafe { TypedUbf::borrowed_from_raw(source.ctx, source.as_ptr()) };
        return clone_ubf(&source, ctx, path).map(TypedUbf::into_inner);
    }
    // VIEW allocation requests can be rounded DOWN to the declared layout.
    // Copy the layout, never the source allocation's spare capacity.
    let size = if info.type_name == "VIEW" {
        ctx.bvsizeof(&info.subtype)?
    } else {
        info.size
    };
    if size > info.size {
        return Err(UbfError::new(
            UbfError::BBADVIEW,
            "VIEW allocation is shorter than its layout",
        ));
    }
    let copy = ctx
        .tpalloc(&info.type_name, &info.subtype, size)
        .map_err(allocation_error)?;
    if copy.tptypes().map_err(allocation_error)?.size < size {
        return Err(UbfError::new(
            UbfError::BNOSPACE,
            "cloned allocation is shorter than the source payload",
        ));
    }
    // Native copying preserves bytes, including struct padding. Pointer CARRAY
    // fields carry no logical length; the clone keeps its tracked length at 0.
    if size != 0 {
        unsafe {
            ptr::copy_nonoverlapping(source.as_ptr(), copy.as_ptr(), size);
        }
    }
    Ok(copy)
}

/// Rebuild a UBF with independent child allocations while rejecting cycles on the active path.
///
/// # Arguments
///
/// - `source`: Borrowed source allocation that remains unchanged.
/// - `ctx`: Context used to allocate or release the affected buffers.
/// - `path`: Addresses on the active recursive clone path, used to detect cycles.
fn clone_ubf<'ctx>(
    source: &TypedUbf<'_>,
    ctx: &'ctx AtmiCtx,
    path: &mut HashSet<usize>,
) -> UbfResult<TypedUbf<'ctx>> {
    let address = source.as_ubfh() as usize;
    if path.len() >= 128 || !path.insert(address) {
        return Err(UbfError::new(
            UbfError::BEINVAL,
            "cyclic or excessively deep UBF pointer graph",
        ));
    }
    let result = (|| {
        let mut copy = ctx
            .tpalloc_ubf(source.ctx().bused(source)?.max(256))
            .map_err(allocation_error)?;
        let mut fields = source.bnext();
        while let Some(field) = fields.next()? {
            match field.field_type {
                UbfFieldType::Ubf => {
                    let nested = source.bget_ubf(field.field_id, field.occurrence)?;
                    let nested =
                        unsafe { TypedUbf::borrowed_from_raw(nested.ctx, nested.ptr.cast()) };
                    copy.put_owned_ubf(
                        field.field_id,
                        field.occurrence,
                        clone_ubf(&nested, ctx, path)?,
                        true,
                    )?;
                }
                UbfFieldType::Ptr => {
                    if source
                        .owned_field_roots(field.field_id, field.occurrence)?
                        .is_empty()
                    {
                        let mut null: *mut c_char = ptr::null_mut();
                        copy.put_native(
                            field.field_id,
                            field.occurrence,
                            (&mut null as *mut *mut c_char).cast(),
                            0,
                            true,
                        )?;
                    } else {
                        let target = source.bget_ptr(field.field_id, field.occurrence)?;
                        copy.bchg(
                            field.field_id,
                            field.occurrence,
                            clone_buffer(&target, ctx, path)?,
                            true,
                        )?;
                    }
                }
                UbfFieldType::View => {
                    let mut len = 0;
                    let found = source.ctx().bfind_value(
                        source,
                        field.field_id,
                        field.occurrence,
                        &mut len,
                    );
                    if found.is_null() {
                        return Err(source.ctx().ubf_last_error());
                    }
                    let mut descriptor =
                        unsafe { ptr::read_unaligned(found.cast::<raw::BVIEWFLD>()) };
                    // Bchg copies the inline VIEW bytes, including empty VIEW
                    // placeholders created before a nonzero occurrence offset.
                    copy.put_native(
                        field.field_id,
                        field.occurrence,
                        (&mut descriptor as *mut raw::BVIEWFLD).cast(),
                        0,
                        true,
                    )?;
                }
                _ => {
                    let mut len = 0;
                    let value = source.ctx().bfind_value(
                        source,
                        field.field_id,
                        field.occurrence,
                        &mut len,
                    );
                    if value.is_null() {
                        return Err(source.ctx().ubf_last_error());
                    }
                    copy.put_native(field.field_id, field.occurrence, value, len, true)?;
                }
            }
        }
        Ok(copy)
    })();
    path.remove(&address);
    result
}

// Native messages can contain shared pointers even though Rust's owned writes
// cannot create them. Refuse destructive edits to those graphs: tpfree cascades
// and would otherwise also free targets reachable from a surviving occurrence.
/// Traverse an ownership graph and reject repeated target addresses or excessive depth.
///
/// # Arguments
///
/// - `ubf`: UBF whose inline children and pointer occurrences are traversed.
/// - `seen`: All target addresses already encountered, used to reject sharing and cycles.
/// - `depth`: Current nesting depth used to enforce the recursion limit.
fn unique_targets(ubf: &TypedUbf<'_>, seen: &mut HashSet<usize>, depth: usize) -> UbfResult<()> {
    if depth >= 128 {
        return Err(UbfError::new(
            UbfError::BEINVAL,
            "UBF ownership nesting exceeds 128 levels",
        ));
    }
    let mut fields = ubf.bnext();
    while let Some(field) = fields.next()? {
        match field.field_type {
            UbfFieldType::Ptr => {
                for target in ubf.owned_field_roots(field.field_id, field.occurrence)? {
                    if !seen.insert(target as usize) {
                        return Err(UbfError::new(UbfError::BEINVAL, "shared/cyclic pointer graph: deep-clone before modifying owned targets"));
                    }
                    let probe = unsafe { TypedBuffer::borrowed_from_raw(ubf.ctx(), target) };
                    if probe.tptypes().map_err(allocation_error)?.type_name == "UBF" {
                        let child = unsafe { TypedUbf::borrowed_from_raw(ubf.ctx(), target) };
                        unique_targets(&child, seen, depth + 1)?;
                    }
                }
            }
            UbfFieldType::Ubf => {
                let child = ubf.bget_ubf(field.field_id, field.occurrence)?;
                let child = unsafe { TypedUbf::borrowed_from_raw(child.ctx, child.ptr.cast()) };
                unique_targets(&child, seen, depth + 1)?;
            }
            _ => (),
        }
    }
    Ok(())
}
