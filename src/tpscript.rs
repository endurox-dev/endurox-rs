//! Safe access to the scripting engine selected by Enduro/X's plugin loader.
use crate::{raw, AtmiCtx, AtmiError, TpScrBuffers, UbfError};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::fmt;
use std::marker::PhantomData;
use std::os::raw::{c_int, c_long};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::rc::Rc;

/// Dispatch to the plain C API or context-aware Object API for the selected feature mode.
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
///
/// Return `Err(TpScrError::new(code, message))` from a Rust callback to report an
/// application error. `?` converts [`AtmiError`] and [`UbfError`] automatically.
/// The callback bridge writes the error to the engine before returning native failure.
/// See the [error propagation example](TpScrVm#returning-and-handling-errors).
///
/// Execution errors are snapshots: inspect their codes and message instead of relying
/// on a later query of mutable native error state. An execution error does not discard
/// the updated buffers in [`TpScrBuffers`], so a failure may still carry an output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpScrError {
    /// Native tperrno, or zero when only the engine reported an error.
    pub atmi_code: u32,
    /// Engine-specific error number. Callback errors are propagated through
    /// native `tpscrseterror` before the trampoline returns failure.
    pub script_code: i32,
    /// Captured diagnostic text from the engine, native API, or Rust callback.
    pub message: String,
}
/// Construction of script errors and binding validation errors.
impl TpScrError {
    /// Create a scripting error with an engine-specific code and no ATMI error code.
    ///
    /// # Arguments
    ///
    /// - `script_code`: Engine-specific error number to report.
    /// - `message`: Diagnostic text to store; native error setters reject embedded NUL bytes.
    pub fn new(script_code: i32, message: impl Into<String>) -> Self {
        Self {
            atmi_code: 0,
            script_code,
            message: message.into(),
        }
    }
    /// Create a scripting argument error carrying ATMI `TPEINVAL`.
    ///
    /// # Arguments
    ///
    /// - `message`: Diagnostic text to store; native error setters reject embedded NUL bytes.
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self {
            atmi_code: raw::TPEINVAL,
            script_code: raw::EXFAIL,
            message: message.into(),
        }
    }
}
/// Human-readable formatting that includes native error codes and diagnostics.
impl fmt::Display for TpScrError {
    /// Format both error codes and the diagnostic message.
    ///
    /// # Arguments
    ///
    /// - `f`: Formatter receiving the error or VM description.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[atmi {}, script {}] {}",
            self.atmi_code, self.script_code, self.message
        )
    }
}
/// Integration with Rust’s standard error trait.
impl std::error::Error for TpScrError {}
/// Conversion that preserves the source value’s native meaning.
impl From<AtmiError> for TpScrError {
    /// Convert an ATMI error while preserving its code in both native and script error fields.
    ///
    /// # Arguments
    ///
    /// - `error`: Native subsystem error to convert without losing its diagnostic.
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
/// Conversion that preserves the source value’s native meaning.
impl From<UbfError> for TpScrError {
    /// Convert a UBF error to a script error, preserving its code and identifying the UBF
    /// diagnostic.
    ///
    /// # Arguments
    ///
    /// - `error`: Native subsystem error to convert without losing its diagnostic.
    fn from(error: UbfError) -> Self {
        Self {
            atmi_code: 0,
            script_code: error.code as i32,
            message: format!("UBF: {}", error.message),
        }
    }
}
/// Result of a scripting operation, preserving ATMI and engine diagnostics.
pub type TpScrResult<T> = Result<T, TpScrError>;

/// Opaque backend-specific configuration. Pass `None` to use engine defaults.
/// The portable C API declares native `tpscr_cfg_t` without defining its layout.
/// Normal initialization uses `ctx.tpscrinit(None, 0)`; constructing this wrapper is
/// only needed when a specific backend documents additional native configuration.
/// See [`TpScrVm`] for setup and usage.
#[derive(Clone, Copy, Debug)]
pub struct TpScrCfg<'a> {
    pointer: *const c_void,
    _borrow: PhantomData<&'a c_void>,
}
/// Unsafe construction of opaque backend configuration borrows.
impl<'a> TpScrCfg<'a> {
    /// Wrap an opaque configuration pointer for the selected scripting backend.
    ///
    /// # Arguments
    ///
    /// - `pointer`: Backend-specific configuration storage whose layout and lifetime satisfy
    ///   that engine’s requirements.
    ///
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
///
/// Obtain it from [`TpScrVm::tpscrcomp`] and pass `&bytecode` directly to
/// [`TpScrVm::tpscrload`]. Use [`Self::as_bytes`] to inspect it or copy the bytes for
/// later loading with a compatible backend. See the [bytecode example](TpScrVm#bytecode-and-load-flags).
#[derive(Debug)]
pub struct TpScrBytecode<'ctx> {
    ctx: &'ctx AtmiCtx,
    pointer: *mut c_char,
    length: usize,
}
/// Read-only access and explicit release of plugin-owned bytecode.
impl TpScrBytecode<'_> {
    /// Borrow the plugin-owned bytecode without copying or transferring ownership.
    pub fn as_bytes(&self) -> &[u8] {
        if self.length == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.pointer.cast(), self.length) }
        }
    }
    /// Explicitly release compiler output. Drop also releases it automatically.
    pub fn tpscrfree(mut self) -> TpScrResult<()> {
        let pointer = std::mem::replace(&mut self.pointer, ptr::null_mut());
        if native!(self.ctx, tpscrfree, Otpscrfree, pointer) == (raw::EXSUCCEED as c_int) {
            Ok(())
        } else {
            Err(self.ctx.atmi_last_error().into())
        }
    }
}
/// Borrowed access to the underlying context or bytecode.
impl AsRef<[u8]> for TpScrBytecode<'_> {
    /// Borrow the bytecode as a byte slice.
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
/// Release remaining compiler output with the scripting plugin’s allocator.
impl Drop for TpScrBytecode<'_> {
    /// Release remaining compiler output with the scripting plugin’s allocator.
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            let _ = native!(self.ctx, tpscrfree, Otpscrfree, self.pointer);
        }
    }
}

/// Rust callbacks may capture owned state. Invocation borrows its context and
/// buffers only for the native callback, so neither can escape into that state.
///
/// Register a closure with [`TpScrVm::tpscrregcb`]. It receives the invoking
/// [`TpScrCallbackContext`] and parameter/input/output [`TpScrBuffers`]. Return
/// `Ok(())` for success or [`TpScrError`] for failure; the output goes in the
/// [`crate::TpScrSlot::Output`] slot. See the [callback example](TpScrVm#rust-callbacks).
pub type TpScrCallback =
    dyn for<'call> Fn(&TpScrCallbackContext<'call>, &mut TpScrBuffers<'call>) -> TpScrResult<()>;

struct CallbackEntry {
    owner: std::thread::ThreadId,
    callback: RefCell<Option<Rc<TpScrCallback>>>,
}

/// An owned Enduro/X scripting VM, wrapping native `tpscr_vm_t`.
///
/// This guide uses the endurox-python backend. Other plugins may use different script
/// entry points and return conventions; the Rust ownership and error APIs stay the same.
///
/// # Setup
///
/// Use a configured Enduro/X environment and install the scripting plugin for the
/// language you need. Native `NDRX_PLUGINS` selects the plugin. For these Python
/// examples, select the installed endurox-python extension and make its matching
/// Python runtime and `endurox` package available; set `PYTHONPATH` when needed.
/// The Rust binding calls the native plugin API rather than starting a Python process.
///
/// Initialize an [`AtmiCtx`] with [`AtmiCtx::tpinit`], then call
/// [`AtmiCtx::tpscrinit`] with `None` and flags `0` for backend defaults.
/// [`TpScrCfg`] is an opaque backend-specific escape hatch, not a required Rust
/// configuration object. A missing scripting plugin makes initialization fail with
/// [`AtmiError::TPERELEASE`] in [`TpScrError::atmi_code`].
///
/// # Quick start
///
/// This complete example sends a CARRAY payload to a Python script and reads its output:
///
/// ```no_run
/// use endurox_rs::{AtmiCtx, TpScrBuffers, TpScrResult, TpScrSlot as S, NDRX_TPSCR_FLAT};
///
/// fn main() -> TpScrResult<()> {
///     let ctx = AtmiCtx::new()?;
///     ctx.tpinit()?;
///     let mut vm = ctx.tpscrinit(None, 0)?;
///
///     vm.tpscrloadstr("echo", r#"
/// def main(name, param, incoming, flags):
///     return incoming
/// "#, NDRX_TPSCR_FLAT)?;
///
///     let mut buffers = TpScrBuffers::new(&ctx);
///     buffers.set(S::Input, Some(ctx.tpalloc_carray(b"hello")?))?;
///     // Param and Output start empty; the script supplies Output through its return value.
///     vm.tpscrexec("echo", &mut buffers, 0)?;
///
///     let output = buffers.take(S::Output)?.expect("echo returned a buffer");
///     assert_eq!(output.as_bytes(), b"hello");
///     Ok(()) // Remaining buffers and the VM are cleaned up automatically.
/// }
/// ```
///
/// The Python backend calls `main(name, param, incoming, flags)` in the loaded script:
///
/// | Python argument | Meaning |
/// | --- | --- |
/// | `name` | Script name passed to [`Self::tpscrexec`], here `"echo"`. |
/// | `param` | Optional application-defined parameter buffer from [`TpScrSlot::Param`](crate::TpScrSlot::Param). |
/// | `incoming` | Input buffer from [`TpScrSlot::Input`](crate::TpScrSlot::Input). |
/// | `flags` | Execution flags supplied to [`Self::tpscrexec`]. |
///
/// An empty input slot arrives as `None`. Python typed buffers use the backend's
/// buffer representation, for example `{"buftype": "CARRAY", "data": b"hello"}`.
/// Returning a buffer supplies the output. Returning `(rc, output)` also supplies a
/// status: `0` is success, and `-1` reports failure. There is no separate Python output
/// argument. Native buffers may be shared between slots; see [`TpScrBuffers`] before
/// retaining or extracting a result. Python views of borrowed UBF arguments are valid
/// only during execution; copy data the script needs to keep for later.
///
/// # Rust callbacks
///
/// Register a callback before executing the script. With endurox-python, its registered
/// name becomes a callable in the script's globals. The call is synchronous:
/// `host(param, incoming, flags)` returns `(rc, output)` to Python.
///
/// Given an initialized `ctx` and `vm`, this callback appends captured Rust text:
///
/// ```no_run
/// # use endurox_rs::{AtmiCtx, TpScrBuffers, TpScrError, TpScrResult, TpScrSlot as S, NDRX_TPSCR_FLAT};
/// # fn example() -> TpScrResult<()> {
/// # let ctx = AtmiCtx::new()?;
/// # ctx.tpinit()?;
/// # let mut vm = ctx.tpscrinit(None, 0)?;
/// let suffix = String::from(" from Rust");
/// vm.tpscrregcb("host", move |call, args| {
///     let mut bytes = args.get(S::Input)?
///         .ok_or_else(|| TpScrError::new(1001, "Input buffer required"))?
///         .as_bytes().to_vec();
///     bytes.extend_from_slice(suffix.as_bytes());
///     let output = call.context().tpalloc_carray(&bytes)?;
///     args.set(S::Output, Some(output))
/// }, 0)?;
///
/// vm.tpscrloadstr("with_host", r#"
/// def main(name, param, incoming, flags):
///     rc, output = host(param, incoming, flags)
///     return rc, output
/// "#, NDRX_TPSCR_FLAT)?;
///
/// let mut buffers = TpScrBuffers::new(&ctx);
/// buffers.set(S::Input, Some(ctx.tpalloc_carray(b"hello")?))?;
/// vm.tpscrexec("with_host", &mut buffers, 0)?;
/// let output = buffers.take(S::Output)?.expect("host returned a buffer");
/// assert_eq!(output.as_bytes(), b"hello from Rust");
/// # Ok(())
/// # }
/// ```
///
/// In a callback, `args` is a [`TpScrBuffers`] scoped to that invocation.
/// [`TpScrCallbackContext::context`] supplies the ATMI context currently executing the
/// script; allocate callback results through it. `call.name()` is the registered
/// callback name, and `call.flags()` contains the flags passed by Python to that callback.
/// The registration flags passed to [`Self::tpscrregcb`] are separate.
///
/// Closures may capture owned state with `move`, including `Rc` and `RefCell` for local
/// mutable state. A closure must not retain borrowed callback contexts or buffers.
/// To return the existing parameter buffer, use
/// `args.alias(S::Output, S::Param)?`; to modify a UBF, use [`TpScrBuffers::edit_ubf`].
/// Use [`Self::tpscrunregcb`] to remove a registration and release its captured state.
///
/// # Returning and handling errors
///
/// Return `Err(TpScrError::new(code, message))` from a Rust callback to fail it. Native
/// ATMI and UBF errors can also propagate with `?`. The binding calls native
/// `tpscrseterror` and returns failure; callback panics are caught and reported too.
/// Setting an error message alone does not fail a callback: its Rust result must be `Err`.
///
/// Python receives a nonzero `rc` from the callback. It can handle that failure or
/// propagate it to the outer [`Self::tpscrexec`] call. This example propagates the
/// status, diagnostic, and an optional failure output explicitly:
///
/// ```no_run
/// # use endurox_rs::{AtmiCtx, TpScrBuffers, TpScrError, TpScrResult, TpScrSlot as S, NDRX_TPSCR_FLAT};
/// # fn example() -> TpScrResult<()> {
/// # let ctx = AtmiCtx::new()?;
/// # ctx.tpinit()?;
/// # let mut vm = ctx.tpscrinit(None, 0)?;
/// vm.tpscrregcb("reject", |call, args| {
///     args.set(S::Output, Some(call.context().tpalloc_carray(b"details")?))?;
///     Err(TpScrError::new(1001, "Invalid request"))
/// }, 0)?;
///
/// vm.tpscrloadstr("checked", r#"
/// def main(name, param, incoming, flags):
///     rc, output = reject(param, incoming, flags)
///     if rc != 0:
///         return {"rc": rc, "out": output,
///                 "screrrno": tpscrerrno(), "error": tpscrerror()}
///     return output
/// "#, NDRX_TPSCR_FLAT)?;
///
/// let mut buffers = TpScrBuffers::new(&ctx);
/// let error = vm.tpscrexec("checked", &mut buffers, 0).unwrap_err();
/// assert_eq!(error.script_code, 1001);
/// assert!(error.message.contains("Invalid request"));
/// // Execution retains buffer updates even when it fails.
/// let output = buffers.take(S::Output)?.expect("failure returned details");
/// assert_eq!(output.as_bytes(), b"details");
/// # Ok(())
/// # }
/// ```
///
/// Use the returned [`TpScrError`] for its captured `atmi_code`, `script_code`, and
/// `message`. Later native calls can change the VM's error state. Unhandled Python
/// exceptions also make execution fail. A failed execution may have modified parameter
/// or input buffers; the write is not transactional.
///
/// # Bytecode and load flags
///
/// Use [`Self::tpscrloadstr`] to load source directly. To compile separately, keep or
/// copy the bytes returned by [`Self::tpscrcomp`], then pass them to [`Self::tpscrload`]:
///
/// ```no_run
/// # use endurox_rs::{AtmiCtx, TpScrResult, NDRX_TPSCR_FLAT};
/// # fn example() -> TpScrResult<()> {
/// # let ctx = AtmiCtx::new()?;
/// # ctx.tpinit()?;
/// # let mut vm = ctx.tpscrinit(None, 0)?;
/// let source = "def main(name, param, incoming, flags):\n    return incoming\n";
/// let bytecode = vm.tpscrcomp(source, 0)?;
/// let saved = bytecode.as_bytes().to_vec();
/// vm.tpscrload("compiled", &bytecode, NDRX_TPSCR_FLAT)?;
/// bytecode.tpscrfree()?; // The loaded script no longer needs this compiler allocation.
/// vm.tpscrunload("compiled", 0)?;
/// vm.tpscrload("compiled", &saved, NDRX_TPSCR_FLAT)?;
/// # Ok(())
/// # }
/// ```
///
/// Bytecode is backend-specific; reload it with a compatible engine and version.
/// [`TpScrBytecode`] releases its plugin-owned memory with native `tpscrfree` when
/// dropped. It may outlive the VM while its borrowed [`AtmiCtx`] remains alive.
///
/// Load flags can be combined with `|`:
///
/// | Flag | Effect for the Python backend |
/// | --- | --- |
/// | `0` | Interpret dotted names as a module hierarchy. |
/// | [`NDRX_TPSCR_PACKAGE`](crate::NDRX_TPSCR_PACKAGE) | Load a package that can contain submodules. |
/// | [`NDRX_TPSCR_REPLACE`](crate::NDRX_TPSCR_REPLACE) | Replace or reload an existing registration. |
/// | [`NDRX_TPSCR_FLAT`](crate::NDRX_TPSCR_FLAT) | Treat the entire name literally, including dots. |
///
/// # Lifetimes and cleanup
///
/// The VM borrows its [`AtmiCtx`] and is neither `Send` nor `Sync`, including with
/// `ctx-send`. Keep VM operations and callbacks on the creating thread. Native calls
/// are synchronous; this API does not use the async reply drivers.
///
/// Dropping the VM shuts it down and releases callback state after successful native
/// shutdown. Call [`Self::tpscruninit`] to consume the VM and inspect shutdown errors.
/// Owned buffers in [`TpScrBuffers`] are freed when the collection is dropped; a buffer
/// removed with [`TpScrBuffers::take`] is freed when its returned owner is dropped.
/// Release the VM and its borrowed buffers before explicitly terminating their context.
pub struct TpScrVm<'ctx> {
    ctx: &'ctx AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    // Boxes give native userdata stable addresses. Keep one entry per registered
    // name until shutdown, even on registration errors/after unregister, since
    // a plugin can partially change registration before reporting failure.
    callbacks: HashMap<String, Box<CallbackEntry>>,
    _config: Option<TpScrCfg<'ctx>>,
    _local: PhantomData<Rc<()>>,
}

/// Creation of scripting VMs through the configured native plugin.
impl AtmiCtx {
    /// Create a VM using the configured native scripting plugin.
    ///
    /// Start with `ctx.tpscrinit(None, 0)` after `ctx.tpinit()` and native plugin
    /// configuration. See the [scripting guide](TpScrVm) for a complete example.
    ///
    /// # Arguments
    ///
    /// - `config`: Opaque backend configuration, or `None` for the selected engine’s defaults.
    /// - `flags`: Native scripting-operation options supported by the selected backend; use `0`
    ///   for defaults.
    pub fn tpscrinit<'ctx>(
        &'ctx self,
        config: Option<TpScrCfg<'ctx>>,
        flags: i64,
    ) -> TpScrResult<TpScrVm<'ctx>> {
        let cfg = config
            .as_ref()
            .map_or(ptr::null(), |cfg| cfg.pointer.cast());
        let pointer = native!(self, tpscrinit, Otpscrinit, cfg, native_flags(flags)?);
        if pointer.is_null() {
            return Err(last_error(self, pointer, "tpscrinit failed", raw::EXFAIL));
        }
        Ok(TpScrVm {
            ctx: self,
            pointer,
            callbacks: HashMap::new(),
            _config: config,
            _local: PhantomData,
        })
    }
}

/// Script compilation, loading, execution, callback registration, and VM cleanup.
impl<'ctx> TpScrVm<'ctx> {
    /// Return the ATMI context borrowed by this VM.
    pub fn context(&self) -> &'ctx AtmiCtx {
        self.ctx
    }

    /// Compile source to plugin-owned bytecode. Embedded NUL bytes are passed
    /// with their explicit length for the engine to accept or reject.
    ///
    /// # Arguments
    ///
    /// - `source`: Source bytes, passed with an explicit byte length for the engine to validate.
    /// - `flags`: Native scripting-operation options supported by the selected backend; use `0`
    ///   for defaults.
    pub fn tpscrcomp(
        &mut self,
        source: impl AsRef<[u8]>,
        flags: i64,
    ) -> TpScrResult<TpScrBytecode<'ctx>> {
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
            return Err(TpScrError::invalid(
                "plugin returned invalid compiler output",
            ));
        }
        Ok(TpScrBytecode {
            ctx: self.ctx,
            pointer,
            length: length as usize,
        })
    }

    /// Load compiled bytecode under a name in this VM.
    ///
    /// # Arguments
    ///
    /// - `name`: Script, package, or callback name understood by the engine; embedded NUL bytes
    ///   are rejected.
    /// - `bytes`: Compiled bytecode compatible with the selected scripting backend.
    /// - `flags`: Load flags: `NDRX_TPSCR_PACKAGE` creates a package, `NDRX_TPSCR_REPLACE`
    ///   replaces an existing registration, and `NDRX_TPSCR_FLAT` keeps dotted names literal.
    pub fn tpscrload(
        &mut self,
        name: &str,
        bytes: impl AsRef<[u8]>,
        flags: i64,
    ) -> TpScrResult<()> {
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

    /// Load source text under a name in this VM, letting the selected engine compile it.
    ///
    /// # Arguments
    ///
    /// - `name`: Script, package, or callback name understood by the engine; embedded NUL bytes
    ///   are rejected.
    /// - `source`: Source bytes, passed with an explicit byte length for the engine to validate.
    /// - `flags`: Load flags: `NDRX_TPSCR_PACKAGE` creates a package, `NDRX_TPSCR_REPLACE`
    ///   replaces an existing registration, and `NDRX_TPSCR_FLAT` keeps dotted names literal.
    pub fn tpscrloadstr(
        &mut self,
        name: &str,
        source: impl AsRef<[u8]>,
        flags: i64,
    ) -> TpScrResult<()> {
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

    /// Unload the script or package registered under a name.
    ///
    /// # Arguments
    ///
    /// - `name`: Script, package, or callback name understood by the engine; embedded NUL bytes
    ///   are rejected.
    /// - `flags`: Native scripting-operation options supported by the selected backend; use `0`
    ///   for defaults.
    pub fn tpscrunload(&mut self, name: &str, flags: i64) -> TpScrResult<()> {
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
    ///
    /// Put a result buffer in [`crate::TpScrSlot::Output`] and return `Ok(())`, or
    /// return `Err(TpScrError::new(code, message))`. The Python backend exposes
    /// the name as `name(param, incoming, flags)`, returning `(rc, output)`.
    /// See the [callback example](TpScrVm#rust-callbacks) and
    /// [error propagation example](TpScrVm#returning-and-handling-errors).
    ///
    /// # Arguments
    ///
    /// - `name`: Script, package, or callback name understood by the engine; embedded NUL bytes
    ///   are rejected.
    /// - `callback`: Synchronous closure receiving the invoking context and mutable buffer
    ///   slots; captured state must be owned.
    /// - `flags`: Native scripting-operation options supported by the selected backend; use `0`
    ///   for defaults.
    pub fn tpscrregcb<F>(&mut self, name: &str, callback: F, flags: i64) -> TpScrResult<()>
    where
        F: for<'call> Fn(&TpScrCallbackContext<'call>, &mut TpScrBuffers<'call>) -> TpScrResult<()>
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

    /// Unregister a host callback and release its closure after native success.
    ///
    /// # Arguments
    ///
    /// - `name`: Script, package, or callback name understood by the engine; embedded NUL bytes
    ///   are rejected.
    /// - `flags`: Native scripting-operation options supported by the selected backend; use `0`
    ///   for defaults.
    pub fn tpscrunregcb(&mut self, name: &str, flags: i64) -> TpScrResult<()> {
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
    ///
    /// Populate [`TpScrBuffers`] first, call this method with the loaded script
    /// name, then inspect or take [`crate::TpScrSlot::Output`]. The call is
    /// synchronous and borrows the collection; it does not consume it.
    /// See the [quick start](TpScrVm#quick-start) and
    /// [error handling example](TpScrVm#returning-and-handling-errors).
    ///
    /// # Arguments
    ///
    /// - `name`: Script, package, or callback name understood by the engine; embedded NUL bytes
    ///   are rejected.
    /// - `buffers`: Parameter/input/output slots from the executing context, updated on success
    ///   and failure.
    /// - `flags`: Native scripting-operation options supported by the selected backend; use `0`
    ///   for defaults.
    pub fn tpscrexec(
        &mut self,
        name: &str,
        buffers: &mut TpScrBuffers<'ctx>,
        flags: i64,
    ) -> TpScrResult<()> {
        execute_value(self.ctx, self.pointer, name, buffers, flags)
    }

    /// Return the selected engine’s current script error number.
    pub fn tpscrerrno(&self) -> i32 {
        native!(self.ctx, tpscrerrno, Otpscrerrno, self.pointer)
    }
    /// Copy the selected engine’s current diagnostic message.
    pub fn tpscrerror(&self) -> String {
        tpscrerror_value(self.ctx, self.pointer)
    }
    /// Set the script error code and diagnostic message visible to the engine.
    ///
    /// This updates error state only. To fail a Rust callback, return an
    /// [`Err`] containing [`TpScrError`]; see [error propagation](TpScrVm#returning-and-handling-errors).
    ///
    /// # Arguments
    ///
    /// - `code`: Engine-specific script error number to set.
    /// - `message`: Diagnostic text to store; native error setters reject embedded NUL bytes.
    pub fn tpscrseterror(&self, code: i32, message: &str) -> TpScrResult<()> {
        tpscrseterror_value(self.ctx, self.pointer, code, message)
    }

    /// Explicitly destroy the VM. Drop destroys it automatically with flags 0.
    /// If the provider fails to destroy it, callback userdata is retained to
    /// avoid dangling pointers in the still-live native VM.
    ///
    /// # Arguments
    ///
    /// - `flags`: Native scripting-operation options supported by the selected backend; use `0`
    ///   for defaults.
    pub fn tpscruninit(mut self, flags: i64) -> TpScrResult<()> {
        let flags = native_flags(flags)?;
        self.shutdown(flags)
    }

    /// Attempt native VM shutdown once, retaining callback userdata if the provider reports
    /// failure.
    ///
    /// # Arguments
    ///
    /// - `flags`: Native scripting-operation options supported by the selected backend; use `0`
    ///   for defaults.
    fn shutdown(&mut self, flags: c_long) -> TpScrResult<()> {
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
/// Attempt VM shutdown with default flags, preserving userdata if shutdown fails.
impl Drop for TpScrVm<'_> {
    /// Attempt VM shutdown with default flags, preserving userdata if shutdown fails.
    fn drop(&mut self) {
        let _ = self.shutdown(0);
    }
}

/// Debug formatting of runtime state without dumping owned native buffers.
impl fmt::Debug for TpScrVm<'_> {
    /// Format the native VM address and registered callback names for debugging.
    ///
    /// # Arguments
    ///
    /// - `f`: Formatter receiving the error or VM description.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TpScrVm")
            .field("pointer", &self.pointer)
            .field("callbacks", &self.callbacks.keys())
            .finish_non_exhaustive()
    }
}

/// Access to the invoking native context and VM during a Rust host callback.
///
/// Obtain this value as the first argument of a closure registered with
/// [`TpScrVm::tpscrregcb`]. Allocate results through [`Self::context`], and return
/// an error through the closure's [`TpScrResult`]. [`Self::tpscrexec`] permits
/// synchronous nested execution when the backend supports it. The context and
/// any borrowed buffers must not escape the invocation.
pub struct TpScrCallbackContext<'call> {
    ctx: &'call AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    name: String,
    flags: i64,
}
/// Access to the invoking context, error state, and nested script execution.
impl<'call> TpScrCallbackContext<'call> {
    /// Borrow the ATMI context currently executing this callback.
    pub fn context(&self) -> &'call AtmiCtx {
        self.ctx
    }
    /// Return the host callback name supplied by the engine.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Return the invocation flags supplied to this callback by the engine.
    pub fn flags(&self) -> i64 {
        self.flags
    }
    /// Return the selected engine’s current script error number.
    pub fn tpscrerrno(&self) -> i32 {
        native!(self.ctx, tpscrerrno, Otpscrerrno, self.pointer)
    }
    /// Copy the selected engine’s current diagnostic message.
    pub fn tpscrerror(&self) -> String {
        tpscrerror_value(self.ctx, self.pointer)
    }
    /// Set the script error code and diagnostic message visible to the engine.
    ///
    /// This updates error state only. To fail a Rust callback, return an
    /// [`Err`] containing [`TpScrError`]; see [error propagation](TpScrVm#returning-and-handling-errors).
    ///
    /// # Arguments
    ///
    /// - `code`: Engine-specific script error number to set.
    /// - `message`: Diagnostic text to store; native error setters reject embedded NUL bytes.
    pub fn tpscrseterror(&self, code: i32, message: &str) -> TpScrResult<()> {
        tpscrseterror_value(self.ctx, self.pointer, code, message)
    }
    /// Reenter the engine if that backend supports nested execution.
    ///
    /// # Arguments
    ///
    /// - `name`: Script, package, or callback name understood by the engine; embedded NUL bytes
    ///   are rejected.
    /// - `buffers`: Parameter/input/output slots from the executing context, updated on success
    ///   and failure.
    /// - `flags`: Native scripting-operation options supported by the selected backend; use `0`
    ///   for defaults.
    pub fn tpscrexec(
        &self,
        name: &str,
        buffers: &mut TpScrBuffers<'call>,
        flags: i64,
    ) -> TpScrResult<()> {
        execute_value(self.ctx, self.pointer, name, buffers, flags)
    }
}

/// Check that a Rust byte length fits the native signed length type.
///
/// # Arguments
///
/// - `length`: Byte count that must fit the native signed `long` length type.
fn native_length(length: usize) -> TpScrResult<c_long> {
    c_long::try_from(length)
        .map_err(|_| TpScrError::invalid("script data length exceeds native long"))
}
/// Check that scripting flags fit the platform’s native `long` type.
///
/// # Arguments
///
/// - `flags`: Native scripting-operation options supported by the selected backend; use `0` for
///   defaults.
fn native_flags(flags: i64) -> TpScrResult<c_long> {
    c_long::try_from(flags).map_err(|_| TpScrError::invalid("script flags exceed native long"))
}
/// Convert a script or callback name to a native string, rejecting embedded NUL bytes.
///
/// # Arguments
///
/// - `name`: Script, package, or callback name understood by the engine; embedded NUL bytes are
///   rejected.
fn name_string(name: &str) -> TpScrResult<CString> {
    CString::new(name).map_err(|_| TpScrError::invalid("script name contains NUL"))
}
/// Copy a provider error string, returning an empty string if the provider returns null.
///
/// # Arguments
///
/// - `_ctx`: ATMI context used for the native error query.
/// - `pointer`: Native VM handle, or null when querying provider thread-local error state.
fn tpscrerror_value(_ctx: &AtmiCtx, pointer: *mut raw::tpscr_vm_t) -> String {
    let message = native!(_ctx, tpscrerror, Otpscrerror, pointer);
    if message.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(message).to_string_lossy().into_owned() }
    }
}
/// Snapshot ATMI and script errors, using the operation text when neither has a diagnostic.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for the operation; it must match the supplied buffers.
/// - `pointer`: Native VM handle, or null when querying provider thread-local error state.
/// - `operation`: Fallback diagnostic if the native layers provide no error message.
/// - `rc`: Native return status; nonzero indicates failure.
fn last_error(
    ctx: &AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    operation: &str,
    rc: i32,
) -> TpScrError {
    // Error-query entry points clear tperrno, so snapshot it first.
    let atmi = ctx.atmi_last_error();
    let code = native!(ctx, tpscrerrno, Otpscrerrno, pointer);
    let message = tpscrerror_value(ctx, pointer);
    TpScrError {
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
/// Translate the native script status to success or a captured script error.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for the operation; it must match the supplied buffers.
/// - `pointer`: Native VM handle, or null when querying provider thread-local error state.
/// - `operation`: Fallback diagnostic if the native layers provide no error message.
/// - `rc`: Native return status; nonzero indicates failure.
fn check(
    ctx: &AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    operation: &str,
    rc: i32,
) -> TpScrResult<()> {
    if rc == (raw::EXSUCCEED as c_int) {
        Ok(())
    } else {
        Err(last_error(ctx, pointer, operation, rc))
    }
}
/// Set a provider error after validating its message as a C string.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for the operation; it must match the supplied buffers.
/// - `pointer`: Native VM handle, or null when querying provider thread-local error state.
/// - `code`: Engine-specific script error number to set.
/// - `message`: Diagnostic text to store; native error setters reject embedded NUL bytes.
fn tpscrseterror_value(
    ctx: &AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    code: i32,
    message: &str,
) -> TpScrResult<()> {
    let message = CString::new(message)
        .map_err(|_| TpScrError::invalid("script error message contains NUL"))?;
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
/// Execute with the collection’s native slots and preserve pointer changes even on failure.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for the operation; it must match the supplied buffers.
/// - `pointer`: Live VM handle used for execution on the invoking thread.
/// - `name`: Script, package, or callback name understood by the engine; embedded NUL bytes are
///   rejected.
/// - `buffers`: Parameter/input/output slots from the executing context, updated on success and
///   failure.
/// - `flags`: Native scripting-operation options supported by the selected backend; use `0` for
///   defaults.
fn execute_value(
    ctx: &AtmiCtx,
    pointer: *mut raw::tpscr_vm_t,
    name: &str,
    buffers: &mut TpScrBuffers<'_>,
    flags: i64,
) -> TpScrResult<()> {
    if !ptr::eq(ctx, buffers.ctx) {
        return Err(TpScrError::invalid(
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

/// Invoke a Rust host closure on the executing context, synchronizing buffers and catching panics.
///
/// # Arguments
///
/// - `vm`: Live native VM invoking the callback.
/// - `name`: Readable NUL-terminated callback name supplied by the provider.
/// - `param`: Mutable parameter-buffer pointer slot, or null if unavailable.
/// - `param_len`: Parameter length slot in bytes, paired with `param`.
/// - `input`: Mutable input-buffer pointer slot, or null if unavailable.
/// - `input_len`: Input length slot in bytes, paired with `input`.
/// - `output`: Mutable output-buffer pointer slot, or null if unavailable.
/// - `output_len`: Output length slot in bytes, paired with `output`.
/// - `user`: Stable `CallbackEntry` pointer installed during callback registration.
/// - `flags`: Invocation options passed through to the Rust callback context.
///
/// # Safety
///
/// The provider must supply live VM, userdata, C-string, and paired buffer slots for a
/// synchronous callback. Allocations may alias only through the tracked slots, and invocation
/// must occur on the registration thread.
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
        let call = TpScrCallbackContext {
            ctx: &ctx,
            pointer: vm,
            name: CStr::from_ptr(name).to_string_lossy().into_owned(),
            flags: flags as i64,
        };
        let pointers = [param, input, output];
        let lengths = [param_len, input_len, output_len];
        let result = (|| {
            let mut buffers = TpScrBuffers::callback(&ctx, pointers, lengths)?;
            let callback = (&*user.cast::<CallbackEntry>()).callback.borrow().clone();
            let result = catch_unwind(AssertUnwindSafe(|| match callback {
                Some(callback) => callback(&call, &mut buffers),
                None => Err(TpScrError::new(
                    raw::EXFAIL,
                    "Rust callback is no longer registered",
                )),
            }))
            .unwrap_or_else(|_| {
                Err(TpScrError::new(
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
