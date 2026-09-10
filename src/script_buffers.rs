//! Ownership of the three mutable buffer slots passed through the scripting API.
use crate::{AtmiCtx, BorrowedBuffer, ScriptError, ScriptResult, TypedBuffer, TypedUbf};
use std::ffi::{c_char, CStr};
use std::os::raw::c_long;
use std::ptr;

/// A scripting argument or result slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptSlot {
    Param = 0,
    Input = 1,
    Output = 2,
}

/// Owns the parameter, input and output of a script call, including aliases.
///
/// Returning an input as output creates two references to one allocation, not
/// two owners. Taking a buffer clears every slot referring to that allocation.
/// Editing a buffer updates all its aliases, including after reallocation or a
/// panic. Buffers remain accessible when script execution returns an error.
#[derive(Debug)]
pub struct ScriptBuffers<'ctx> {
    pub(crate) ctx: &'ctx AtmiCtx,
    pub(crate) pointers: [*mut c_char; 3],
    pub(crate) lengths: [c_long; 3],
    links: [usize; 3],
    enabled: [bool; 3],
    owned: bool,
    native_slots: Option<([*mut *mut c_char; 3], [*mut c_long; 3])>,
}

impl<'ctx> ScriptBuffers<'ctx> {
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

    pub fn context(&self) -> &'ctx AtmiCtx {
        self.ctx
    }

    /// Replace one slot. The displaced allocation is freed only if no other
    /// slot refers to it. Callback slots sharing one native pointer are linked.
    pub fn set(&mut self, slot: ScriptSlot, buffer: Option<TypedBuffer<'ctx>>) -> ScriptResult<()> {
        let index = slot as usize;
        if !self.enabled[index] {
            return Err(ScriptError::invalid(
                "this callback buffer slot was not supplied",
            ));
        }
        let (next, len) = match buffer {
            Some(buffer) => {
                if !ptr::eq(buffer.ctx, self.ctx) {
                    return Err(ScriptError::invalid(
                        "script buffer belongs to a different context",
                    ));
                }
                let len = c_long::try_from(buffer.len())
                    .map_err(|_| ScriptError::invalid("buffer length exceeds native long"))?;
                (buffer.into_raw(), len)
            }
            None => (ptr::null_mut(), 0),
        };
        self.replace(index, next, len);
        Ok(())
    }

    /// Make `destination` refer to the same allocation as `source`.
    pub fn alias(&mut self, destination: ScriptSlot, source: ScriptSlot) -> ScriptResult<()> {
        let destination = destination as usize;
        if !self.enabled[destination] {
            return Err(ScriptError::invalid(
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
    pub fn get(&self, slot: ScriptSlot) -> ScriptResult<Option<BorrowedBuffer<'_, 'ctx>>> {
        let index = slot as usize;
        if self.pointers[index].is_null() {
            return Ok(None);
        }
        let length = usize::try_from(self.lengths[index])
            .map_err(|_| ScriptError::invalid("negative native script buffer length"))?;
        let mut buffer = unsafe { TypedBuffer::borrowed_from_raw(self.ctx, self.pointers[index]) };
        let info = buffer.tptypes()?;
        if length > info.size {
            return Err(ScriptError::invalid(
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
    pub fn take(&mut self, slot: ScriptSlot) -> ScriptResult<Option<TypedBuffer<'ctx>>> {
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
    pub fn edit<R>(
        &mut self,
        slot: ScriptSlot,
        edit: impl FnOnce(&mut TypedBuffer<'ctx>) -> ScriptResult<R>,
    ) -> ScriptResult<R> {
        let aliases = self.aliases(slot);
        let buffer = self
            .take(slot)?
            .ok_or_else(|| ScriptError::invalid("script buffer slot is empty"))?;
        let mut writeback = BufferWriteback {
            slots: self,
            aliases,
            buffer: Some(buffer),
        };
        edit(writeback.buffer.as_mut().unwrap())
    }

    /// Edit UBF fields, allowing growth while preserving parameter/output aliases.
    pub fn edit_ubf<R>(
        &mut self,
        slot: ScriptSlot,
        edit: impl FnOnce(&mut TypedUbf<'ctx>) -> ScriptResult<R>,
    ) -> ScriptResult<R> {
        let buffer = self
            .get(slot)?
            .ok_or_else(|| ScriptError::invalid("script buffer slot is empty"))?;
        if buffer.tptypes()?.type_name != "UBF" {
            return Err(ScriptError::invalid("script buffer is not UBF"));
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

    fn aliases(&self, slot: ScriptSlot) -> [bool; 3] {
        let pointer = self.pointers[slot as usize];
        self.pointers.map(|p| !pointer.is_null() && p == pointer)
    }

    // Temporarily take responsibility for native callback slots. If user code
    // replaces/drops the entire collection, Drop clears the real slots before
    // freeing their allocations. Normal return hands ownership back to C.
    pub(crate) unsafe fn callback(
        ctx: &'ctx AtmiCtx,
        pointers: [*mut *mut c_char; 3],
        lengths: [*mut c_long; 3],
    ) -> ScriptResult<Self> {
        for i in 0..3 {
            if pointers[i].is_null() != lengths[i].is_null() {
                return Err(ScriptError::invalid(
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

    pub(crate) unsafe fn return_to_native(
        &mut self,
        ctx: &AtmiCtx,
        pointers: [*mut *mut c_char; 3],
        lengths: [*mut c_long; 3],
    ) -> ScriptResult<()> {
        if !ptr::eq(ctx, self.ctx) {
            return Err(ScriptError::invalid(
                "callback returned buffers from another context",
            ));
        }
        for i in 0..3 {
            if pointers[i].is_null() && !self.pointers[i].is_null() {
                return Err(ScriptError::invalid(
                    "callback returned an unavailable buffer slot",
                ));
            }
            for j in 0..i {
                if !pointers[i].is_null()
                    && pointers[i] == pointers[j]
                    && (self.pointers[i] != self.pointers[j] || self.lengths[i] != self.lengths[j])
                {
                    return Err(ScriptError::invalid(
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

impl Drop for ScriptBuffers<'_> {
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

trait ScriptOwnedBuffer<'ctx> {
    fn into_buffer(self) -> TypedBuffer<'ctx>;
}
impl<'ctx> ScriptOwnedBuffer<'ctx> for TypedBuffer<'ctx> {
    fn into_buffer(self) -> TypedBuffer<'ctx> {
        self
    }
}
impl<'ctx> ScriptOwnedBuffer<'ctx> for TypedUbf<'ctx> {
    fn into_buffer(self) -> TypedBuffer<'ctx> {
        self.into_inner()
    }
}

struct BufferWriteback<'a, 'ctx, B: ScriptOwnedBuffer<'ctx>> {
    slots: &'a mut ScriptBuffers<'ctx>,
    aliases: [bool; 3],
    buffer: Option<B>,
}
impl<'ctx, B: ScriptOwnedBuffer<'ctx>> Drop for BufferWriteback<'_, 'ctx, B> {
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
