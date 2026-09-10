//! Safe access to the scripting engine selected by Enduro/X's plugin loader.
use crate::{raw, AtmiCtx, AtmiError, ScriptBuffers, UbfError};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::fmt;
use std::marker::PhantomData;
use std::os::raw::{c_int, c_long};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::rc::Rc;

macro_rules! native {
    ($ctx:expr, $plain:ident, $object:ident $(, $arg:expr)* $(,)?) => {{
        #[cfg(not(feature = "ctx-send"))]
        let result = unsafe { raw::$plain($($arg),*) };
        #[cfg(feature = "ctx-send")]
        let result = unsafe { raw::$object($ctx.c_ctx_ptr(), $($arg),*) };
        result
    }};
}

/// An error from the native XATMI layer or the selected scripting engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    /// Native tperrno, or zero when only the engine reported an error.
    pub atmi_code: u32,
    /// Engine-specific error number. Callback errors are propagated through
    /// native `tpscrseterror` before the trampoline returns failure.
    pub script_code: i32,
    pub message: String,
}
impl ScriptError {
    pub fn new(script_code: i32, message: impl Into<String>) -> Self {
        Self {
            atmi_code: 0,
            script_code,
            message: message.into(),
        }
    }
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self {
            atmi_code: raw::TPEINVAL,
            script_code: raw::EXFAIL,
            message: message.into(),
        }
    }
}
impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[atmi {}, script {}] {}",
            self.atmi_code, self.script_code, self.message
        )
    }
}
impl std::error::Error for ScriptError {}
impl From<AtmiError> for ScriptError {
    fn from(error: AtmiError) -> Self {
        Self {
            atmi_code: error.code,
            // The callback ABI carries script errors, so preserve the native
            // error number there as well when a Rust host operation fails.
            script_code: error.code as i32,
            message: error.message.into_owned(),
        }
    }
}
impl From<UbfError> for ScriptError {
    fn from(error: UbfError) -> Self {
        Self {
            atmi_code: 0,
            script_code: error.code as i32,
            message: format!("UBF: {}", error.message),
        }
    }
}
pub type ScriptResult<T> = Result<T, ScriptError>;

/// Opaque backend-specific configuration. Pass `None` to use engine defaults.
/// The portable C API does not define a configuration layout.
#[derive(Clone, Copy, Debug)]
pub struct ScriptConfig<'a> {
    pointer: *const c_void,
    _borrow: PhantomData<&'a c_void>,
}
impl<'a> ScriptConfig<'a> {
    /// # Safety
    /// `pointer` must have the layout required by the selected engine and remain
    /// valid for `'a`, including any engine retention until the VM is destroyed.
    pub unsafe fn from_raw(pointer: *const c_void) -> Self {
        Self {
            pointer,
            _borrow: PhantomData,
        }
    }
}

/// Compiler output owned by the scripting plugin, freed with `tpscrfree`.
#[derive(Debug)]
pub struct ScriptBytecode<'ctx> {
    ctx: &'ctx AtmiCtx,
    pointer: *mut c_char,
    length: usize,
}
impl ScriptBytecode<'_> {
    pub fn as_bytes(&self) -> &[u8] {
        if self.length == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.pointer.cast(), self.length) }
        }
    }
    /// Explicitly release compiler output. Drop also releases it automatically.
    pub fn tpscrfree(mut self) -> ScriptResult<()> {
        let pointer = std::mem::replace(&mut self.pointer, ptr::null_mut());
        if native!(self.ctx, tpscrfree, Otpscrfree, pointer) == (raw::EXSUCCEED as c_int) {
            Ok(())
        } else {
            Err(self.ctx.atmi_last_error().into())
        }
    }
}
impl AsRef<[u8]> for ScriptBytecode<'_> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl Drop for ScriptBytecode<'_> {
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            let _ = native!(self.ctx, tpscrfree, Otpscrfree, self.pointer);
        }
    }
}

/// Rust callbacks may capture owned state. Invocation borrows its context and
/// buffers only for the native callback, so neither can escape into that state.
pub type ScriptCallback =
    dyn for<'call> Fn(&ScriptCallbackContext<'call>, &mut ScriptBuffers<'call>) -> ScriptResult<()>;

struct CallbackEntry {
    owner: std::thread::ThreadId,
    callback: RefCell<Option<Rc<ScriptCallback>>>,
}

/// An owned scripting VM. It is local to one thread, even with `ctx-send`.
///
/// The engine is selected by native `NDRX_PLUGINS`; this wrapper does not embed
/// a particular language. Native callbacks must be synchronous on the invoking
/// thread, as required by the borrowed callback context/buffer interface.
pub struct ScriptVm<'ctx> {
    ctx: &'ctx AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    // Boxes give native userdata stable addresses. Keep one entry per registered
    // name until shutdown, even on registration errors/after unregister, since
    // a plugin can partially change registration before reporting failure.
    callbacks: HashMap<String, Box<CallbackEntry>>,
    _config: Option<ScriptConfig<'ctx>>,
    _local: PhantomData<Rc<()>>,
}

impl AtmiCtx {
    /// Create a VM using the configured native scripting plugin.
    pub fn tpscrinit<'ctx>(
        &'ctx self,
        config: Option<ScriptConfig<'ctx>>,
        flags: i64,
    ) -> ScriptResult<ScriptVm<'ctx>> {
        let cfg = config
            .as_ref()
            .map_or(ptr::null(), |cfg| cfg.pointer.cast());
        let pointer = native!(self, tpscrinit, Otpscrinit, cfg, native_flags(flags)?);
        if pointer.is_null() {
            return Err(last_error(self, pointer, "tpscrinit failed", raw::EXFAIL));
        }
        Ok(ScriptVm {
            ctx: self,
            pointer,
            callbacks: HashMap::new(),
            _config: config,
            _local: PhantomData,
        })
    }
}

impl<'ctx> ScriptVm<'ctx> {
    pub fn context(&self) -> &'ctx AtmiCtx {
        self.ctx
    }

    /// Compile source to plugin-owned bytecode. Embedded NUL bytes are passed
    /// with their explicit length for the engine to accept or reject.
    pub fn tpscrcomp(
        &mut self,
        source: impl AsRef<[u8]>,
        flags: i64,
    ) -> ScriptResult<ScriptBytecode<'ctx>> {
        let source = source.as_ref();
        let mut pointer = ptr::null_mut();
        let mut length: c_long = 0;
        let rc = native!(
            self.ctx,
            tpscrcomp,
            Otpscrcomp,
            self.pointer,
            source.as_ptr().cast(),
            native_length(source.len())?,
            &mut pointer,
            &mut length,
            native_flags(flags)?
        );
        if rc != (raw::EXSUCCEED as c_int) {
            let error = last_error(self.ctx, self.pointer, "tpscrcomp failed", rc);
            if !pointer.is_null() {
                let _ = native!(self.ctx, tpscrfree, Otpscrfree, pointer);
            }
            return Err(error);
        }
        if length < 0 || length as u128 > isize::MAX as u128 || (pointer.is_null() && length != 0) {
            if !pointer.is_null() {
                let _ = native!(self.ctx, tpscrfree, Otpscrfree, pointer);
            }
            return Err(ScriptError::invalid(
                "plugin returned invalid compiler output",
            ));
        }
        Ok(ScriptBytecode {
            ctx: self.ctx,
            pointer,
            length: length as usize,
        })
    }

    pub fn tpscrload(
        &mut self,
        name: &str,
        bytes: impl AsRef<[u8]>,
        flags: i64,
    ) -> ScriptResult<()> {
        let name = name_string(name)?;
        let bytes = bytes.as_ref();
        let rc = native!(
            self.ctx,
            tpscrload,
            Otpscrload,
            self.pointer,
            name.as_ptr(),
            bytes.as_ptr().cast(),
            native_length(bytes.len())?,
            native_flags(flags)?
        );
        check(self.ctx, self.pointer, "tpscrload failed", rc)
    }

    pub fn tpscrloadstr(
        &mut self,
        name: &str,
        source: impl AsRef<[u8]>,
        flags: i64,
    ) -> ScriptResult<()> {
        let name = name_string(name)?;
        let source = source.as_ref();
        let rc = native!(
            self.ctx,
            tpscrloadstr,
            Otpscrloadstr,
            self.pointer,
            name.as_ptr(),
            source.as_ptr().cast(),
            native_length(source.len())?,
            native_flags(flags)?
        );
        check(self.ctx, self.pointer, "tpscrloadstr failed", rc)
    }

    pub fn tpscrunload(&mut self, name: &str, flags: i64) -> ScriptResult<()> {
        let name = name_string(name)?;
        let rc = native!(
            self.ctx,
            tpscrunload,
            Otpscrunload,
            self.pointer,
            name.as_ptr(),
            native_flags(flags)?
        );
        check(self.ctx, self.pointer, "tpscrunload failed", rc)
    }

    /// Register or replace a host callback, with captured Rust state.
    pub fn tpscrregcb<F>(&mut self, name: &str, callback: F, flags: i64) -> ScriptResult<()>
    where
        F: for<'call> Fn(
                &ScriptCallbackContext<'call>,
                &mut ScriptBuffers<'call>,
            ) -> ScriptResult<()>
            + 'static,
    {
        let cname = name_string(name)?;
        let flags = native_flags(flags)?;
        let entry = self.callbacks.entry(name.to_owned()).or_insert_with(|| {
            Box::new(CallbackEntry {
                owner: std::thread::current().id(),
                callback: RefCell::new(None),
            })
        });
        let userdata = (&mut **entry as *mut CallbackEntry).cast();
        let rc = native!(
            self.ctx,
            tpscrregcb,
            Otpscrregcb,
            self.pointer,
            cname.as_ptr(),
            Some(callback_value),
            userdata,
            flags
        );
        check(self.ctx, self.pointer, "tpscrregcb failed", rc)?;
        *entry.callback.borrow_mut() = Some(Rc::new(callback));
        Ok(())
    }

    pub fn tpscrunregcb(&mut self, name: &str, flags: i64) -> ScriptResult<()> {
        let cname = name_string(name)?;
        let rc = native!(
            self.ctx,
            tpscrunregcb,
            Otpscrunregcb,
            self.pointer,
            cname.as_ptr(),
            native_flags(flags)?
        );
        check(self.ctx, self.pointer, "tpscrunregcb failed", rc)?;
        if let Some(entry) = self.callbacks.get(name) {
            *entry.callback.borrow_mut() = None;
        }
        Ok(())
    }

    /// Execute a script, adopting all returned pointers and lengths even on error.
    pub fn tpscrexec(
        &mut self,
        name: &str,
        buffers: &mut ScriptBuffers<'ctx>,
        flags: i64,
    ) -> ScriptResult<()> {
        execute_value(self.ctx, self.pointer, name, buffers, flags)
    }

    pub fn tpscrerrno(&self) -> i32 {
        native!(self.ctx, tpscrerrno, Otpscrerrno, self.pointer)
    }
    pub fn tpscrerror(&self) -> String {
        tpscrerror_value(self.ctx, self.pointer)
    }
    pub fn tpscrseterror(&self, code: i32, message: &str) -> ScriptResult<()> {
        tpscrseterror_value(self.ctx, self.pointer, code, message)
    }

    /// Explicitly destroy the VM. Drop destroys it automatically with flags 0.
    /// If the provider fails to destroy it, callback userdata is retained to
    /// avoid dangling pointers in the still-live native VM.
    pub fn tpscruninit(mut self, flags: i64) -> ScriptResult<()> {
        let flags = native_flags(flags)?;
        self.shutdown(flags)
    }

    fn shutdown(&mut self, flags: c_long) -> ScriptResult<()> {
        if self.pointer.is_null() {
            return Ok(());
        }
        let rc = native!(self.ctx, tpscruninit, Otpscruninit, self.pointer, flags);
        // A failing finalizer may already have consumed the VM. Query the
        // provider's thread-local error rather than dereferencing that handle.
        let result = check(self.ctx, ptr::null_mut(), "tpscruninit failed", rc);
        self.pointer = ptr::null_mut();
        if result.is_err() {
            std::mem::forget(std::mem::take(&mut self.callbacks));
        }
        result
    }
}
impl Drop for ScriptVm<'_> {
    fn drop(&mut self) {
        let _ = self.shutdown(0);
    }
}

impl fmt::Debug for ScriptVm<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScriptVm")
            .field("pointer", &self.pointer)
            .field("callbacks", &self.callbacks.keys())
            .finish_non_exhaustive()
    }
}

/// Access to the invoking native context and VM during a Rust host callback.
pub struct ScriptCallbackContext<'call> {
    ctx: &'call AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    name: String,
    flags: i64,
}
impl<'call> ScriptCallbackContext<'call> {
    pub fn context(&self) -> &'call AtmiCtx {
        self.ctx
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn flags(&self) -> i64 {
        self.flags
    }
    pub fn tpscrerrno(&self) -> i32 {
        native!(self.ctx, tpscrerrno, Otpscrerrno, self.pointer)
    }
    pub fn tpscrerror(&self) -> String {
        tpscrerror_value(self.ctx, self.pointer)
    }
    pub fn tpscrseterror(&self, code: i32, message: &str) -> ScriptResult<()> {
        tpscrseterror_value(self.ctx, self.pointer, code, message)
    }
    /// Reenter the engine if that backend supports nested execution.
    pub fn tpscrexec(
        &self,
        name: &str,
        buffers: &mut ScriptBuffers<'call>,
        flags: i64,
    ) -> ScriptResult<()> {
        execute_value(self.ctx, self.pointer, name, buffers, flags)
    }
}

fn native_length(length: usize) -> ScriptResult<c_long> {
    c_long::try_from(length)
        .map_err(|_| ScriptError::invalid("script data length exceeds native long"))
}
fn native_flags(flags: i64) -> ScriptResult<c_long> {
    c_long::try_from(flags).map_err(|_| ScriptError::invalid("script flags exceed native long"))
}
fn name_string(name: &str) -> ScriptResult<CString> {
    CString::new(name).map_err(|_| ScriptError::invalid("script name contains NUL"))
}
fn tpscrerror_value(_ctx: &AtmiCtx, pointer: *mut raw::tpscr_vm_t) -> String {
    let message = native!(_ctx, tpscrerror, Otpscrerror, pointer);
    if message.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(message).to_string_lossy().into_owned() }
    }
}
fn last_error(
    ctx: &AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    operation: &str,
    rc: i32,
) -> ScriptError {
    // Error-query entry points clear tperrno, so snapshot it first.
    let atmi = ctx.atmi_last_error();
    let code = native!(ctx, tpscrerrno, Otpscrerrno, pointer);
    let message = tpscrerror_value(ctx, pointer);
    ScriptError {
        atmi_code: atmi.code,
        script_code: if code == 0 { rc } else { code },
        message: if !message.is_empty() {
            message
        } else if atmi.code != 0 {
            atmi.message.into_owned()
        } else {
            operation.to_owned()
        },
    }
}
fn check(
    ctx: &AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    operation: &str,
    rc: i32,
) -> ScriptResult<()> {
    if rc == (raw::EXSUCCEED as c_int) {
        Ok(())
    } else {
        Err(last_error(ctx, pointer, operation, rc))
    }
}
fn tpscrseterror_value(
    ctx: &AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    code: i32,
    message: &str,
) -> ScriptResult<()> {
    let message = CString::new(message)
        .map_err(|_| ScriptError::invalid("script error message contains NUL"))?;
    let rc = native!(
        ctx,
        tpscrseterror,
        Otpscrseterror,
        pointer,
        code,
        message.as_ptr()
    );
    check(ctx, pointer, "tpscrseterror failed", rc)
}
fn execute_value(
    ctx: &AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    name: &str,
    buffers: &mut ScriptBuffers<'_>,
    flags: i64,
) -> ScriptResult<()> {
    if !ptr::eq(ctx, buffers.ctx) {
        return Err(ScriptError::invalid(
            "script buffers belong to a different context",
        ));
    }
    let name = name_string(name)?;
    let flags = native_flags(flags)?;
    let p = buffers.pointers.as_mut_ptr();
    let n = buffers.lengths.as_mut_ptr();
    // The arrays hold the actual ownership slots. Native reallocations/aliases
    // are written back directly, including on script exceptions.
    let rc = native!(
        ctx,
        tpscrexec,
        Otpscrexec,
        pointer,
        name.as_ptr(),
        p,
        n,
        p.add(1),
        n.add(1),
        p.add(2),
        n.add(2),
        flags
    );
    buffers.sync_native();
    check(ctx, pointer, "tpscrexec failed", rc)
}

#[allow(clippy::too_many_arguments, clippy::unnecessary_cast)]
unsafe extern "C" fn callback_value(
    vm: *mut raw::tpscr_vm_t,
    name: *const c_char,
    param: *mut *mut c_char,
    param_len: *mut c_long,
    input: *mut *mut c_char,
    input_len: *mut c_long,
    output: *mut *mut c_char,
    output_len: *mut c_long,
    user: *mut c_void,
    flags: c_long,
) -> c_int {
    if user.is_null() || vm.is_null() || name.is_null() {
        return raw::EXFAIL;
    }
    if (*user.cast::<CallbackEntry>()).owner != std::thread::current().id() {
        return raw::EXFAIL;
    }
    catch_unwind(AssertUnwindSafe(|| {
        // Detach native TLS while Rust executes. Nested Object API operations
        // can then lock it normally; Drop restores it for the enclosing call.
        let ctx = match AtmiCtx::borrow_current_context() {
            Ok(ctx) => ctx,
            Err(_) => return raw::EXFAIL,
        };
        let call = ScriptCallbackContext {
            ctx: &ctx,
            pointer: vm,
            name: CStr::from_ptr(name).to_string_lossy().into_owned(),
            flags: flags as i64,
        };
        let pointers = [param, input, output];
        let lengths = [param_len, input_len, output_len];
        let result = (|| {
            let mut buffers = ScriptBuffers::callback(&ctx, pointers, lengths)?;
            let callback = (&*user.cast::<CallbackEntry>()).callback.borrow().clone();
            let result = catch_unwind(AssertUnwindSafe(|| match callback {
                Some(callback) => callback(&call, &mut buffers),
                None => Err(ScriptError::new(
                    raw::EXFAIL,
                    "Rust callback is no longer registered",
                )),
            }))
            .unwrap_or_else(|_| {
                Err(ScriptError::new(
                    raw::EXFAIL,
                    "Rust scripting callback panicked",
                ))
            });
            // Write back even on panic/error, before the provider resumes its
            // own ownership cleanup. Identical pointer slots are kept linked.
            buffers.return_to_native(&ctx, pointers, lengths)?;
            result
        })();
        match result {
            Ok(()) => raw::EXSUCCEED as c_int,
            Err(error) => {
                let _ = call.tpscrseterror(error.script_code, &error.message.replace('\0', "\\0"));
                raw::EXFAIL
            }
        }
    }))
    .unwrap_or(raw::EXFAIL)
}
