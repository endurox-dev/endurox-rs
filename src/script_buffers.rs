//! Ownership of the three mutable buffer slots passed through the scripting API.
use crate::{AtmiCtx, BorrowedBuffer, TpScrError, TpScrResult, TypedBuffer, TypedUbf};
use std::ffi::{c_char, CStr};
use std::os::raw::c_long;
use std::ptr;

/// A scripting argument or result slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TpScrSlot {
    /// Application-defined parameter buffer; native `param` and `param_len`.
    Param = 0,
    /// Main input payload; native `idata` and `ilen`.
    Input = 1,
    /// Result buffer; native `odata` and `olen`.
    Output = 2,
}

/// Owns the parameter, input, and output buffer slots of a script call, including aliases.
///
/// Start with the [scripting guide](crate::TpScrVm) for a complete call. This is a Rust
/// ownership helper: native `tpscrexec` still receives three separate buffer-pointer
/// and length pairs, rather than a C buffer-container structure.
///
/// | Slot | Native arguments | Purpose |
/// | --- | --- | --- |
/// | [`TpScrSlot::Param`] | `param`, `param_len` | Application-defined parameters for this invocation. |
/// | [`TpScrSlot::Input`] | `idata`, `ilen` | Main request payload. |
/// | [`TpScrSlot::Output`] | `odata`, `olen` | Script or callback result, optionally pre-populated. |
///
/// [`Self::new`] starts with all slots empty. Use [`Self::set`] to move an owned
/// [`TypedBuffer`] into a slot, or pass `None` to clear it. For a UBF or VIEW, pass its
/// `into_inner()` result. All buffers must belong to the collection's context.
/// The collection remains accessible after [`crate::TpScrVm::tpscrexec`] succeeds
/// or fails, including any pointer and length changes made by the backend.
///
/// # Aliases and ownership
///
/// Two slots may reference the same allocation. [`Self::alias`] establishes this
/// explicitly in Rust; a native call may also return a parameter or input allocation
/// as output. Sharing is recognized by equal native pointer addresses, not by equal
/// contents. Setting an unrelated new output keeps the input allocation separate.
///
/// [`Self::get`] borrows without moving or freeing a buffer. [`Self::take`] transfers
/// ownership out and clears every slot referencing that allocation. Dropping the
/// collection frees each distinct allocation still held exactly once. For example:
///
/// ```no_run
/// use endurox_rs::{AtmiCtx, TpScrBuffers, TpScrResult, TpScrSlot as S};
///
/// # fn example() -> TpScrResult<()> {
/// let ctx = AtmiCtx::new()?;
/// ctx.tpinit()?;
/// let mut buffers = TpScrBuffers::new(&ctx);
/// buffers.set(S::Param, Some(ctx.tpalloc_carray(b"parameters")?))?;
/// buffers.alias(S::Output, S::Param)?; // Both slots now reference the same allocation.
///
/// let output = buffers.take(S::Output)?.expect("parameter buffer is present");
/// assert!(buffers.get(S::Param)?.is_none());
/// assert!(buffers.get(S::Output)?.is_none());
/// drop(buffers); // The extracted allocation is no longer in the collection.
/// assert_eq!(output.as_bytes(), b"parameters");
/// drop(output);  // TypedBuffer frees the allocation with native tpfree.
/// # Ok(())
/// # }
/// ```
///
/// # Editing buffers
///
/// Use [`Self::edit`] for a generic typed buffer and [`Self::edit_ubf`] for UBF fields.
/// The closure gets exclusive access; all aliases are restored afterward, including
/// a new pointer after reallocation. An error or panic restores ownership and aliases,
/// but does not undo edits that already happened.
///
/// This example grows a CARRAY and preserves its output alias:
///
/// ```no_run
/// # use endurox_rs::{AtmiCtx, TpScrBuffers, TpScrResult, TpScrSlot as S};
/// # fn example() -> TpScrResult<()> {
/// # let ctx = AtmiCtx::new()?;
/// # ctx.tpinit()?;
/// # let mut buffers = TpScrBuffers::new(&ctx);
/// buffers.set(S::Input, Some(ctx.tpalloc_carray(b"small")?))?;
/// buffers.alias(S::Output, S::Input)?;
/// buffers.edit(S::Input, |buffer| {
///     buffer.set_bytes(&vec![b'x'; 32_768])?;
///     Ok(())
/// })?;
/// assert_eq!(buffers.get(S::Output)?.unwrap().as_bytes().len(), 32_768);
/// # Ok(())
/// # }
/// ```
///
/// Byte views expose the CARRAY payload and NUL-terminated STRING/JSON contents,
/// including the terminator. UBF and VIEW byte views are empty because their native
/// layouts are not raw payloads. Use [`Self::edit_ubf`] for UBF reads or writes, or
/// [`Self::take`] and [`crate::TypedUbf::from_typed`] / [`crate::TypedView::from_typed`]
/// when you need a standalone typed owner.
///
/// # Inside callbacks
///
/// Rust callbacks receive a collection scoped to the invocation. Allocate replacements
/// through [`crate::TpScrCallbackContext::context`] and write the result to
/// [`TpScrSlot::Output`]. To return the parameter itself, call
/// `args.alias(TpScrSlot::Output, TpScrSlot::Param)?`. Normal callback return transfers
/// the remaining buffers back to the native provider, including on error or panic.
/// If an optional native slot was not supplied at all, writing to it returns an error.
/// See the [callback example](crate::TpScrVm#rust-callbacks).
#[derive(Debug)]
pub struct TpScrBuffers<'ctx> {
    pub(crate) ctx: &'ctx AtmiCtx,
    pub(crate) pointers: [*mut c_char; 3],
    pub(crate) lengths: [c_long; 3],
    links: [usize; 3],
    enabled: [bool; 3],
    owned: bool,
    native_slots: Option<([*mut *mut c_char; 3], [*mut c_long; 3])>,
}

/// Ownership-preserving access to parameter, input, and output buffer slots.
impl<'ctx> TpScrBuffers<'ctx> {
    /// Create empty parameter, input, and output slots borrowing an ATMI context.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context borrowed by all allocations in the collection.
    pub fn new(ctx: &'ctx AtmiCtx) -> Self {
        Self {
            ctx,
            pointers: [ptr::null_mut(); 3],
            lengths: [0; 3],
            links: [0, 1, 2],
            enabled: [true; 3],
            owned: true,
            native_slots: None,
        }
    }

    /// Return the context borrowed by this buffer collection.
    pub fn context(&self) -> &'ctx AtmiCtx {
        self.ctx
    }

    /// Replace one slot. The displaced allocation is freed only if no other
    /// slot refers to it. Callback slots sharing one native pointer are linked.
    ///
    /// # Arguments
    ///
    /// - `slot`: Parameter, input, or output slot to access.
    /// - `buffer`: Owned replacement from the same context, or `None` to clear the slot.
    pub fn set(&mut self, slot: TpScrSlot, buffer: Option<TypedBuffer<'ctx>>) -> TpScrResult<()> {
        let index = slot as usize;
        if !self.enabled[index] {
            return Err(TpScrError::invalid(
                "this callback buffer slot was not supplied",
            ));
        }
        let (next, len) = match buffer {
            Some(buffer) => {
                if !ptr::eq(buffer.ctx, self.ctx) {
                    return Err(TpScrError::invalid(
                        "script buffer belongs to a different context",
                    ));
                }
                let len = c_long::try_from(buffer.len())
                    .map_err(|_| TpScrError::invalid("buffer length exceeds native long"))?;
                (buffer.into_raw(), len)
            }
            None => (ptr::null_mut(), 0),
        };
        self.replace(index, next, len);
        Ok(())
    }

    /// Make `destination` refer to the same allocation as `source`.
    ///
    /// This shares the allocation without copying it. The displaced destination
    /// is freed only when no slot still refers to it. See
    /// [aliases and ownership](TpScrBuffers#aliases-and-ownership).
    ///
    /// # Arguments
    ///
    /// - `destination`: Slot to replace with a reference to the source allocation.
    /// - `source`: Slot whose current allocation and length are shared; an empty source clears
    ///   the destination.
    pub fn alias(&mut self, destination: TpScrSlot, source: TpScrSlot) -> TpScrResult<()> {
        let destination = destination as usize;
        if !self.enabled[destination] {
            return Err(TpScrError::invalid(
                "this callback buffer slot was not supplied",
            ));
        }
        self.replace(
            destination,
            self.pointers[source as usize],
            self.lengths[source as usize],
        );
        Ok(())
    }

    /// Update a slot and its native links, freeing the old allocation after its last reference
    /// is removed.
    ///
    /// # Arguments
    ///
    /// - `index`: Slot index: parameter is 0, input is 1, and output is 2.
    /// - `next`: New native allocation pointer, or null to clear the linked slots.
    /// - `length`: Native payload length in bytes to record with the replacement pointer.
    fn replace(&mut self, index: usize, next: *mut c_char, length: c_long) {
        let old = self.pointers[index];
        for i in 0..3 {
            if self.links[i] == self.links[index] {
                self.pointers[i] = next;
                self.lengths[i] = length;
            }
        }
        self.sync_native();
        if !old.is_null() && !self.pointers.contains(&old) {
            drop(unsafe { TypedBuffer::from_raw(self.ctx, old) });
        }
    }

    /// Borrow a buffer without allowing it to be moved or reallocated.
    ///
    /// Returns `None` for an empty slot. Use [`Self::edit`] or [`Self::edit_ubf`]
    /// to change it, and [`Self::take`] to obtain a standalone owner. UBF/VIEW
    /// byte views are empty; read their contents through typed access.
    ///
    /// # Arguments
    ///
    /// - `slot`: Parameter, input, or output slot to access.
    pub fn get(&self, slot: TpScrSlot) -> TpScrResult<Option<BorrowedBuffer<'_, 'ctx>>> {
        let index = slot as usize;
        if self.pointers[index].is_null() {
            return Ok(None);
        }
        let length = usize::try_from(self.lengths[index])
            .map_err(|_| TpScrError::invalid("negative native script buffer length"))?;
        let mut buffer = unsafe { TypedBuffer::borrowed_from_raw(self.ctx, self.pointers[index]) };
        let info = buffer.tptypes()?;
        if length > info.size {
            return Err(TpScrError::invalid(
                "native script buffer length exceeds its allocation",
            ));
        }
        // Engines can report UBF allocation capacity after growth. Structured
        // layouts/padding are not a byte payload. Strings instead have a native
        // NUL-terminated initialized prefix; never expose their spare capacity.
        let initialized_length = match info.type_name.as_str() {
            "CARRAY" => length,
            "STRING" | "JSON" => unsafe {
                CStr::from_ptr(buffer.as_ptr()).to_bytes_with_nul().len()
            },
            _ => 0,
        };
        buffer.set_len_reported(initialized_length);
        Ok(Some(unsafe { BorrowedBuffer::from_unowned(buffer) }))
    }

    /// Take ownership, clearing every alias of the selected allocation.
    ///
    /// Returns `None` for an empty slot. The returned owner frees the allocation
    /// when dropped; the collection no longer owns it. Taking an output that
    /// aliases the parameter also clears the parameter slot. See the
    /// [ownership example](TpScrBuffers#aliases-and-ownership).
    ///
    /// # Arguments
    ///
    /// - `slot`: Parameter, input, or output slot to access.
    pub fn take(&mut self, slot: TpScrSlot) -> TpScrResult<Option<TypedBuffer<'ctx>>> {
        let Some(buffer) = self.get(slot)? else {
            return Ok(None);
        };
        let pointer = buffer.as_ptr();
        let length = buffer.len();
        for i in 0..3 {
            if self.pointers[i] == pointer {
                self.pointers[i] = ptr::null_mut();
                self.lengths[i] = 0;
            }
        }
        self.sync_native();
        Ok(Some(unsafe {
            TypedBuffer::from_raw_with_len(self.ctx, pointer, length)
        }))
    }

    /// Edit a buffer and keep all aliases synchronized, including on errors.
    ///
    /// # Arguments
    ///
    /// - `slot`: Parameter, input, or output slot to access.
    /// - `edit`: Closure given exclusive access to the selected buffer; its result is returned
    ///   after alias restoration.
    pub fn edit<R>(
        &mut self,
        slot: TpScrSlot,
        edit: impl FnOnce(&mut TypedBuffer<'ctx>) -> TpScrResult<R>,
    ) -> TpScrResult<R> {
        let aliases = self.aliases(slot);
        let buffer = self
            .take(slot)?
            .ok_or_else(|| TpScrError::invalid("script buffer slot is empty"))?;
        let mut writeback = BufferWriteback {
            slots: self,
            aliases,
            buffer: Some(buffer),
        };
        edit(writeback.buffer.as_mut().unwrap())
    }

    /// Edit UBF fields, allowing growth while preserving parameter/output aliases.
    ///
    /// # Arguments
    ///
    /// - `slot`: Parameter, input, or output slot to access.
    /// - `edit`: Closure given exclusive access to the selected buffer; its result is returned
    ///   after alias restoration.
    pub fn edit_ubf<R>(
        &mut self,
        slot: TpScrSlot,
        edit: impl FnOnce(&mut TypedUbf<'ctx>) -> TpScrResult<R>,
    ) -> TpScrResult<R> {
        let buffer = self
            .get(slot)?
            .ok_or_else(|| TpScrError::invalid("script buffer slot is empty"))?;
        if buffer.tptypes()?.type_name != "UBF" {
            return Err(TpScrError::invalid("script buffer is not UBF"));
        }
        let aliases = self.aliases(slot);
        let buffer = TypedUbf::from_typed(self.take(slot)?.unwrap())?;
        let mut writeback = BufferWriteback {
            slots: self,
            aliases,
            buffer: Some(buffer),
        };
        edit(writeback.buffer.as_mut().unwrap())
    }

    /// Return which slots refer to the same non-null allocation as the selected slot.
    ///
    /// # Arguments
    ///
    /// - `slot`: Parameter, input, or output slot to access.
    fn aliases(&self, slot: TpScrSlot) -> [bool; 3] {
        let pointer = self.pointers[slot as usize];
        self.pointers.map(|p| !pointer.is_null() && p == pointer)
    }

    // Temporarily take responsibility for native callback slots. If user code
    // replaces/drops the entire collection, Drop clears the real slots before
    // freeing their allocations. Normal return hands ownership back to C.
    /// Adopt callback buffer slots temporarily, tracking shared native pointer slots for writeback.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context borrowed by all allocations in the collection.
    /// - `pointers`: Provider pointer slots in parameter/input/output order; null entries
    ///   denote unavailable slots.
    /// - `lengths`: Matching provider length slots in bytes; each must be null exactly when its
    ///   pointer slot is null.
    ///
    /// # Safety
    ///
    /// Supplied pointer/length slots and their native allocations must remain valid for the
    /// callback lifetime. Their temporary Rust ownership must be exclusive except for tracked
    /// aliases.
    pub(crate) unsafe fn callback(
        ctx: &'ctx AtmiCtx,
        pointers: [*mut *mut c_char; 3],
        lengths: [*mut c_long; 3],
    ) -> TpScrResult<Self> {
        for i in 0..3 {
            if pointers[i].is_null() != lengths[i].is_null() {
                return Err(TpScrError::invalid(
                    "callback pointer and length slots must be paired",
                ));
            }
        }
        let mut buffers = Self::new(ctx);
        for i in 0..3 {
            buffers.enabled[i] = !pointers[i].is_null();
            if buffers.enabled[i] {
                buffers.pointers[i] = *pointers[i];
                buffers.lengths[i] = *lengths[i];
                for j in 0..i {
                    if pointers[i] == pointers[j] {
                        buffers.links[i] = buffers.links[j];
                    }
                }
            }
        }
        buffers.native_slots = Some((pointers, lengths));
        Ok(buffers)
    }

    /// Validate the returned slots and transfer their allocation ownership back to the provider.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context borrowed by all allocations in the collection.
    /// - `pointers`: Provider pointer slots in parameter/input/output order; null entries
    ///   denote unavailable slots.
    /// - `lengths`: Matching provider length slots in bytes; each must be null exactly when its
    ///   pointer slot is null.
    ///
    /// # Safety
    ///
    /// The paired provider slots must remain writable. Successful transfer makes the provider
    /// responsible for all returned allocations; Rust must not free them afterward.
    pub(crate) unsafe fn return_to_native(
        &mut self,
        ctx: &AtmiCtx,
        pointers: [*mut *mut c_char; 3],
        lengths: [*mut c_long; 3],
    ) -> TpScrResult<()> {
        if !ptr::eq(ctx, self.ctx) {
            return Err(TpScrError::invalid(
                "callback returned buffers from another context",
            ));
        }
        for i in 0..3 {
            if pointers[i].is_null() && !self.pointers[i].is_null() {
                return Err(TpScrError::invalid(
                    "callback returned an unavailable buffer slot",
                ));
            }
            for j in 0..i {
                if !pointers[i].is_null()
                    && pointers[i] == pointers[j]
                    && (self.pointers[i] != self.pointers[j] || self.lengths[i] != self.lengths[j])
                {
                    return Err(TpScrError::invalid(
                        "callback returned conflicting aliases for one native slot",
                    ));
                }
            }
        }
        for i in 0..3 {
            if !pointers[i].is_null() {
                *pointers[i] = self.pointers[i];
                *lengths[i] = self.lengths[i];
            }
        }
        self.owned = false;
        Ok(())
    }

    // Update real provider-owned pointer slots before reentering a script. In
    // particular, a Python UbfDict may point directly at one of these slots.
    /// Write current pointers and lengths into the provider’s slots before script reentry.
    pub(crate) fn sync_native(&self) {
        if let Some((pointers, lengths)) = self.native_slots {
            for i in 0..3 {
                if !pointers[i].is_null() {
                    unsafe {
                        *pointers[i] = self.pointers[i];
                        *lengths[i] = self.lengths[i];
                    }
                }
            }
        }
    }
}

/// Clear linked native slots and free each remaining owned allocation exactly once.
impl Drop for TpScrBuffers<'_> {
    /// Clear linked native slots and free each remaining owned allocation exactly once.
    fn drop(&mut self) {
        if !self.owned {
            return;
        }
        let pointers = self.pointers;
        self.pointers = [ptr::null_mut(); 3];
        self.lengths = [0; 3];
        self.sync_native();
        for i in 0..3 {
            let pointer = pointers[i];
            if !pointer.is_null() && !pointers[..i].contains(&pointer) {
                drop(unsafe { TypedBuffer::from_raw(self.ctx, pointer) });
            }
        }
    }
}

/// Convert an edited buffer wrapper back into the owned type stored in script slots.
trait TpScrOwnedBuffer<'ctx> {
    /// Consume a typed wrapper and return the owned generic buffer for slot writeback.
    fn into_buffer(self) -> TypedBuffer<'ctx>;
}
/// Ownership transfer from this wrapper into a script buffer slot.
impl<'ctx> TpScrOwnedBuffer<'ctx> for TypedBuffer<'ctx> {
    /// Consume a typed wrapper and return the owned generic buffer for slot writeback.
    fn into_buffer(self) -> TypedBuffer<'ctx> {
        self
    }
}
/// Ownership transfer from this wrapper into a script buffer slot.
impl<'ctx> TpScrOwnedBuffer<'ctx> for TypedUbf<'ctx> {
    /// Consume a typed wrapper and return the owned generic buffer for slot writeback.
    fn into_buffer(self) -> TypedBuffer<'ctx> {
        self.into_inner()
    }
}

/// Restore all aliases when an edit finishes, fails, or unwinds.
struct BufferWriteback<'a, 'ctx, B: TpScrOwnedBuffer<'ctx>> {
    slots: &'a mut TpScrBuffers<'ctx>,
    aliases: [bool; 3],
    buffer: Option<B>,
}
/// Ownership transfer from this wrapper into a script buffer slot.
impl<'ctx, B: TpScrOwnedBuffer<'ctx>> Drop for BufferWriteback<'_, 'ctx, B> {
    /// Restore the edited buffer to every saved alias and synchronize the native slots.
    fn drop(&mut self) {
        let buffer = self.buffer.take().unwrap().into_buffer();
        let length = buffer.len() as c_long;
        let pointer = buffer.into_raw();
        for i in 0..3 {
            if self.aliases[i] {
                self.slots.pointers[i] = pointer;
                self.slots.lengths[i] = length;
            }
        }
        self.slots.sync_native();
    }
}
