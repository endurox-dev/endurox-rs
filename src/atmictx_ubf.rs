//! Native UBF operations, field identifiers, expression evaluation, and callback I/O.
use crate::raw::*;
use crate::{raw, AtmiCtx, BorrowedUbf, TypedUbf, UbfResult};
use core::ffi::{c_char, c_int, c_long, c_void};
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};

/// Fast-add location state used by [`TypedUbf::fast_adder`].
///
/// The native cursor caches a position *inside* a specific buffer allocation.
/// It is invalidated by anything that relocates that allocation, and it is
/// meaningless against a different buffer. `owner` records which allocation the
/// cursor currently refers to so both cases can be detected and the cursor
/// restarted instead of dereferencing a stale position.
#[derive(Debug)]
pub struct BFldLocInfo {
    pub(crate) inner: raw::Bfld_loc_info_t,
    pub(crate) owner: *mut c_char,
}

/// Cleared native initialization for `BFldLocInfo`.
impl Default for BFldLocInfo {
    /// Create an unpositioned fast-append cursor with no buffer owner.
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
            owner: std::ptr::null_mut(),
        }
    }
}

/// Management of fast-append cursor state tied to a specific allocation.
impl BFldLocInfo {
    /// Restart the cursor against `buffer`.
    ///
    /// # Arguments
    ///
    /// - `buffer`: Native buffer allocation associated with the cursor.
    pub(crate) fn rebase(&mut self, buffer: *mut c_char) {
        self.inner = unsafe { std::mem::zeroed() };
        self.owner = buffer;
    }

    /// Whether this cursor is currently positioned in `buffer`.
    ///
    /// # Arguments
    ///
    /// - `buffer`: Native buffer allocation associated with the cursor.
    pub(crate) fn belongs_to(&self, buffer: *mut c_char) -> bool {
        !self.owner.is_null() && self.owner == buffer
    }
}

/// Compiled UBF boolean expression tree.
#[derive(Debug)]
pub struct UbfExprTree<'ctx> {
    ptr: *mut c_char,
    ctx: &'ctx AtmiCtx,
}

/// Access and explicit cleanup of an owned compiled expression.
impl<'ctx> UbfExprTree<'ctx> {
    /// Borrow the compiled expression’s native pointer without transferring ownership.
    #[inline]
    pub(crate) fn as_ptr(&self) -> *mut c_char {
        self.ptr
    }

    /// Free the compiled expression once and clear its pointer.
    fn free(&mut self) {
        if !self.ptr.is_null() {
            self.ctx.btreefree_value(self.ptr);
            self.ptr = std::ptr::null_mut();
        }
    }
}

/// Release the owned native expression tree.
impl Drop for UbfExprTree<'_> {
    /// Release the owned native expression tree.
    fn drop(&mut self) {
        self.free();
    }
}

/// UBF expression callback registered with `Bboolsetcbf`.
///
/// Receives the evaluated UBF and registered function name; returns a native integer
/// expression value. The UBF is borrowed only for this synchronous invocation.
pub type UbfExprCallback = fn(&TypedUbf<'_>, &str) -> i64;

/// UBF expression callback registered with `Bboolsetcbf2`.
///
/// Receives the evaluated UBF, registered function name, and the expression’s string
/// argument; returns a native integer expression value.
pub type UbfExprCallback2 = fn(&TypedUbf<'_>, &str, &str) -> i64;

struct OutputState {
    bytes: Vec<u8>,
}

struct ReadState<'a> {
    bytes: &'a [u8],
    offset: usize,
}

/// Return the process-wide registry for expression callbacks without a string argument.
fn expr_callbacks() -> &'static Mutex<HashMap<String, UbfExprCallback>> {
    static CALLBACKS: OnceLock<Mutex<HashMap<String, UbfExprCallback>>> = OnceLock::new();
    CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Return the process-wide registry for expression callbacks with a string argument.
fn expr_callbacks2() -> &'static Mutex<HashMap<String, UbfExprCallback2>> {
    static CALLBACKS: OnceLock<Mutex<HashMap<String, UbfExprCallback2>>> = OnceLock::new();
    CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()))
}

thread_local! {
    static EXPR_CONTEXT: Cell<*const AtmiCtx> = const { Cell::new(std::ptr::null()) };
}

/// Keep the evaluator's borrow alive while C synchronously calls back into Rust.
/// Restoring the previous pointer also supports nested evaluations on another
/// context, including when a callback panics.
struct ExprEvaluation<'ctx> {
    _ctx: &'ctx AtmiCtx,
    previous: *const AtmiCtx,
}

/// Scoped association of an expression evaluation with its ATMI context.
impl<'ctx> ExprEvaluation<'ctx> {
    /// Make the evaluating context available to synchronous callbacks until the guard is dropped.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context whose native evaluation is active for the lifetime of the guard.
    fn enter(ctx: &'ctx AtmiCtx) -> Self {
        Self {
            _ctx: ctx,
            previous: EXPR_CONTEXT.with(|current| current.replace(ctx)),
        }
    }
}

/// Restore the context that was active before this expression evaluation.
impl Drop for ExprEvaluation<'_> {
    /// Restore the context that was active before this expression evaluation.
    fn drop(&mut self) {
        EXPR_CONTEXT.with(|current| current.set(self.previous));
    }
}

/// OUBF attaches and locks NSTD/UBF TLS without attaching ATMI TLS. Detach
/// those locks during user code so nested Object API calls can acquire them,
/// then restore them before returning to the native evaluator. ATMI TLS must
/// be saved separately: a server dispatcher may already have it attached,
/// whereas an ordinary OUBF call need not have any ATMI TLS attached at all.
#[cfg(feature = "ctx-send")]
struct ExprCallbackTls<'ctx> {
    ctx: &'ctx AtmiCtx,
    previous_atmi: raw::TPCONTEXT_T,
    ubf_detached: bool,
}

/// Temporary release of native TLS locks during reentrant Rust callbacks.
#[cfg(feature = "ctx-send")]
impl<'ctx> ExprCallbackTls<'ctx> {
    const ATMI_FLAGS: c_long =
        (raw::CTXT_PRIV_ATMI | raw::CTXT_PRIV_TRAN | raw::CTXT_PRIV_IGN) as c_long;
    const UBF_FLAGS: c_long =
        (raw::CTXT_PRIV_NSTD | raw::CTXT_PRIV_UBF | raw::CTXT_PRIV_IGN) as c_long;

    // SAFETY: called only from an expression callback within ExprEvaluation.
    /// Temporarily release native TLS locks so a Rust expression callback can make nested
    /// Object API calls.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context whose native evaluation is active for the lifetime of the guard.
    ///
    /// # Safety
    ///
    /// Call only from a synchronous native expression callback within an active
    /// `ExprEvaluation` for `ctx`.
    unsafe fn suspend(ctx: &'ctx AtmiCtx) -> Option<Self> {
        let mut previous_atmi = std::ptr::null_mut();
        if raw::ndrx_tpgetctxt(&mut previous_atmi, 0, Self::ATMI_FLAGS) == raw::EXFAIL {
            return None;
        }
        let mut guard = Self {
            ctx,
            previous_atmi,
            ubf_detached: false,
        };
        if raw::ndrx_tpgetctxt(ctx.c_ctx_ptr(), 0, Self::UBF_FLAGS) != raw::TPMULTICONTEXTS as c_int
        {
            return None;
        }
        guard.ubf_detached = true;
        Some(guard)
    }
}

/// Restore the native TLS components temporarily detached for a Rust callback.
#[cfg(feature = "ctx-send")]
impl Drop for ExprCallbackTls<'_> {
    /// Restore the native TLS components temporarily detached for a Rust callback.
    fn drop(&mut self) {
        // The evaluator still borrows the context, so these handles remain
        // valid. Restore only the components that were attached on entry.
        unsafe {
            if self.ubf_detached {
                let _ = raw::ndrx_tpsetctxt(*self.ctx.c_ctx_ptr(), 0, Self::UBF_FLAGS);
            }
            if !self.previous_atmi.is_null() {
                let _ = raw::ndrx_tpsetctxt(self.previous_atmi, 0, Self::ATMI_FLAGS);
            }
        }
    }
}

/// Append a native text-print chunk while merging its trailing NUL with the previous chunk.
///
/// # Arguments
///
/// - `buffer`: Readable pointer slot containing the native print chunk.
/// - `datalen`: Number of readable bytes in the print chunk, including its native terminator.
/// - `dataptr1`: Non-null pointer to the mutable `OutputState` collecting text.
/// - `_do_write`: Unused native output switch; output is captured in Rust.
/// - `_outf`: Unused native FILE destination.
/// - `_fid`: Unused native field identifier for the printed chunk.
///
/// # Safety
///
/// Non-null input slots must be valid for the indicated length, and userdata must exclusively
/// reference a live `OutputState`.
unsafe extern "C" fn bfprint_output_callback(
    buffer: *mut *mut c_char,
    datalen: c_long,
    dataptr1: *mut c_void,
    _do_write: *mut c_int,
    _outf: *mut raw::FILE,
    _fid: c_int,
) -> c_int {
    if buffer.is_null() {
        return raw::EXFAIL;
    }
    if (*buffer).is_null() || dataptr1.is_null() || datalen < 0 {
        return raw::EXFAIL;
    }

    let state = &mut *(dataptr1 as *mut OutputState);
    if !state.bytes.is_empty() {
        state.bytes.pop();
    }
    let data = std::slice::from_raw_parts(*buffer as *const u8, datalen as usize);
    state.bytes.extend_from_slice(data);
    raw::EXSUCCEED as c_int
}

/// Copy the next available input bytes into the native reader’s destination.
///
/// # Arguments
///
/// - `buffer`: Writable destination with at least `bufsz` bytes.
/// - `bufsz`: Destination capacity in bytes.
/// - `dataptr1`: Non-null pointer to `ReadState`, whose offset advances as bytes are copied.
///
/// # Safety
///
/// The output must be writable for `bufsz` bytes and userdata must exclusively reference a live
/// `ReadState`.
unsafe extern "C" fn read_callback(
    buffer: *mut c_char,
    bufsz: c_long,
    dataptr1: *mut c_void,
) -> c_long {
    if buffer.is_null() || dataptr1.is_null() || bufsz <= 0 {
        return 0;
    }

    let state = &mut *(dataptr1 as *mut ReadState<'_>);
    if state.offset >= state.bytes.len() {
        return 0;
    }

    let remaining = state.bytes.len() - state.offset;
    let to_copy = remaining.min(bufsz as usize);
    std::ptr::copy_nonoverlapping(
        state.bytes[state.offset..].as_ptr(),
        buffer as *mut u8,
        to_copy,
    );
    state.offset += to_copy;
    to_copy as c_long
}

/// Append a native binary-output chunk to the Rust output vector.
///
/// # Arguments
///
/// - `buffer`: Readable native output chunk of `bufsz` bytes.
/// - `bufsz`: Readable chunk length in bytes.
/// - `dataptr1`: Non-null pointer to the mutable `OutputState` collecting bytes.
///
/// # Safety
///
/// The input must be readable for `bufsz` bytes and userdata must exclusively reference a live
/// `OutputState`.
unsafe extern "C" fn write_callback(
    buffer: *mut c_char,
    bufsz: c_long,
    dataptr1: *mut c_void,
) -> c_long {
    if buffer.is_null() || dataptr1.is_null() || bufsz < 0 {
        return raw::EXFAIL as c_long;
    }

    let state = &mut *(dataptr1 as *mut OutputState);
    let data = std::slice::from_raw_parts(buffer as *const u8, bufsz as usize);
    state.bytes.extend_from_slice(data);
    bufsz
}

/// Dispatch a native expression callback without an explicit string argument.
///
/// # Arguments
///
/// - `p_ub`: Live UBF being evaluated; it remains owned by the native caller.
/// - `funcname`: Readable NUL-terminated native name identifying the registered callback.
///
/// # Safety
///
/// Invoke synchronously inside `ExprEvaluation`; the UBF and function-name C string must remain
/// valid for the callback.
unsafe extern "C" fn expr_callback_proxy(p_ub: *mut raw::UBFH, funcname: *mut c_char) -> c_long {
    expr_callback_proxy_impl(p_ub, funcname, std::ptr::null_mut())
}

/// Dispatch a native expression callback with one string argument.
///
/// # Arguments
///
/// - `p_ub`: Live UBF being evaluated; it remains owned by the native caller.
/// - `funcname`: Readable NUL-terminated native name identifying the registered callback.
/// - `arg1`: Nullable NUL-terminated callback argument; null selects the callback without an
///   argument.
///
/// # Safety
///
/// Invoke synchronously inside `ExprEvaluation`; the UBF, function-name string, and optional
/// argument must remain readable.
unsafe extern "C" fn expr_callback_proxy2(
    p_ub: *mut raw::UBFH,
    funcname: *mut c_char,
    arg1: *mut c_char,
) -> c_long {
    expr_callback_proxy_impl(p_ub, funcname, arg1)
}

/// Resolve and invoke an expression callback on the evaluating context, returning zero on panic.
///
/// # Arguments
///
/// - `p_ub`: Live UBF being evaluated; it remains owned by the native caller.
/// - `funcname`: Readable NUL-terminated native name identifying the registered callback.
/// - `arg1`: Nullable NUL-terminated callback argument; null selects the callback without an
///   argument.
///
/// # Safety
///
/// All non-null native arguments must remain valid during the callback, with the evaluator’s
/// context active in `EXPR_CONTEXT`.
unsafe fn expr_callback_proxy_impl(
    p_ub: *mut raw::UBFH,
    funcname: *mut c_char,
    arg1: *mut c_char,
) -> c_long {
    if p_ub.is_null() || funcname.is_null() {
        return 0;
    }

    let name = CStr::from_ptr(funcname).to_string_lossy().into_owned();

    // Resolve the entry and release the registry lock *before* invoking the
    // callback: holding it across user code would deadlock a callback that
    // registers another one.
    let arg = if arg1.is_null() {
        None
    } else {
        Some(CStr::from_ptr(arg1).to_string_lossy().into_owned())
    };

    let resolved = if arg.is_none() {
        expr_callbacks()
            .lock()
            .ok()
            .and_then(|callbacks| callbacks.get(&name).copied())
            .map(Callback::One)
    } else {
        expr_callbacks2()
            .lock()
            .ok()
            .and_then(|callbacks| callbacks.get(&name).copied())
            .map(Callback::Two)
    };

    let Some(callback) = resolved else {
        return 0;
    };
    let ctx_ptr = EXPR_CONTEXT.with(Cell::get);
    if ctx_ptr.is_null() {
        return 0;
    }

    // SAFETY: ExprEvaluation holds this borrow for the synchronous native
    // evaluation on this thread. The callback's higher-ranked argument cannot
    // retain it, and nested evaluations restore the previous borrow on exit.
    catch_unwind(AssertUnwindSafe(|| {
        let ctx = &*ctx_ptr;
        #[cfg(feature = "ctx-send")]
        let Some(_tls) = ExprCallbackTls::suspend(ctx) else {
            return 0;
        };
        let ubf = TypedUbf::borrowed_from_raw(ctx, p_ub as *mut c_char);
        match (callback, &arg) {
            (Callback::One(cb), _) => cb(&ubf, &name),
            (Callback::Two(cb), Some(arg)) => cb(&ubf, &name, arg),
            (Callback::Two(_), None) => 0,
        }
    }))
    .unwrap_or(0) as c_long
}

/// Either arity of registered expression callback.
#[derive(Clone, Copy)]
enum Callback {
    One(UbfExprCallback),
    Two(UbfExprCallback2),
}

/// UBF field type for safe field-id construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UbfFieldType {
    /// `BFLD_SHORT`.
    Short,
    /// `BFLD_LONG`.
    Long,
    /// `BFLD_CHAR`.
    Char,
    /// `BFLD_FLOAT`.
    Float,
    /// `BFLD_DOUBLE`.
    Double,
    /// `BFLD_STRING`.
    String,
    /// `BFLD_CARRAY`.
    Carray,
    /// `BFLD_PTR`.
    Ptr,
    /// `BFLD_UBF`.
    Ubf,
    /// `BFLD_VIEW`.
    View,
}

/// Conversion between Rust field kinds and native `BFLD_*` codes.
impl UbfFieldType {
    /// Convert a Rust field kind to its native `BFLD_*` constant.
    #[inline]
    fn as_raw(self) -> c_int {
        match self {
            UbfFieldType::Short => raw::BFLD_SHORT as c_int,
            UbfFieldType::Long => raw::BFLD_LONG as c_int,
            UbfFieldType::Char => raw::BFLD_CHAR as c_int,
            UbfFieldType::Float => raw::BFLD_FLOAT as c_int,
            UbfFieldType::Double => raw::BFLD_DOUBLE as c_int,
            UbfFieldType::String => raw::BFLD_STRING as c_int,
            UbfFieldType::Carray => raw::BFLD_CARRAY as c_int,
            UbfFieldType::Ptr => raw::BFLD_PTR as c_int,
            UbfFieldType::Ubf => raw::BFLD_UBF as c_int,
            UbfFieldType::View => raw::BFLD_VIEW as c_int,
        }
    }

    /// Convert a native field type, returning `None` for unknown type codes.
    ///
    /// # Arguments
    ///
    /// - `raw_type`: Native `BFLD_*` type code to convert.
    ///
    #[inline]
    pub(crate) fn from_raw(raw_type: c_int) -> Option<Self> {
        match raw_type as u32 {
            raw::BFLD_SHORT => Some(UbfFieldType::Short),
            raw::BFLD_LONG => Some(UbfFieldType::Long),
            raw::BFLD_CHAR => Some(UbfFieldType::Char),
            raw::BFLD_FLOAT => Some(UbfFieldType::Float),
            raw::BFLD_DOUBLE => Some(UbfFieldType::Double),
            raw::BFLD_STRING => Some(UbfFieldType::String),
            raw::BFLD_CARRAY => Some(UbfFieldType::Carray),
            raw::BFLD_PTR => Some(UbfFieldType::Ptr),
            raw::BFLD_UBF => Some(UbfFieldType::Ubf),
            raw::BFLD_VIEW => Some(UbfFieldType::View),
            _ => None,
        }
    }
}

/// UBF field operations and native expression APIs for this context.
impl AtmiCtx {
    /// Convert a native UBF status to success or the current UBF error.
    ///
    /// # Arguments
    ///
    /// - `rc`: Native return status or count to translate.
    ///
    #[inline]
    fn ubf_unit_result(&self, rc: c_int) -> UbfResult<()> {
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.ubf_last_error())
        }
    }

    /// Convert a nonnegative native count to `usize`, or return the current UBF error.
    ///
    /// # Arguments
    ///
    /// - `rc`: Native return status or count to translate.
    ///
    #[inline]
    fn ubf_count_result<T>(&self, rc: T) -> UbfResult<usize>
    where
        T: Into<i64> + Copy,
    {
        let value = rc.into();
        if value < 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(value as usize)
        }
    }

    /// Borrow the native UBF error-number slot for this context.
    #[inline]
    pub(crate) fn ndrx_bget_ferror_addr(&self) -> *mut c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::ndrx_Bget_Ferror_addr()
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::Ondrx_Bget_Ferror_addr(self.c_ctx_ptr())
        }
    }

    /// Borrow the native diagnostic string for a UBF error code.
    ///
    /// # Arguments
    ///
    /// - `err`: Native UBF error number to describe.
    ///
    #[inline]
    pub(crate) fn bstrerror(&self, err: c_int) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bstrerror(err)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBstrerror(self.c_ctx_ptr(), err)
        }
    }

    /// Copy an inline UBF occurrence through the native API without managing pointer ownership.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `occ`: Zero-based field occurrence to access.
    /// - `value`: Child UBF whose inline bytes are copied; the caller must enforce pointer
    ///   ownership rules.
    ///
    #[inline]
    pub(crate) fn bchg_ubf_value(
        &self,
        ubf: &mut TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        value: &TypedUbf<'_>,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bchg(
                ubf.as_ubfh(),
                bfldid,
                occ,
                value.as_ubfh() as *mut c_char,
                0,
            )
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBchg(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                occ,
                value.as_ubfh() as *mut c_char,
                0,
            )
        }
    }

    /// Append a value with native type conversion and return the unmodified C status.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `buf`: Readable native value storage matching `usrtype`; pointer fields require the
    ///   address of a pointer variable.
    /// - `len`: Value length in bytes for variable-length input; use the native type’s
    ///   convention for fixed-size values.
    /// - `usrtype`: Native `BFLD_*` type describing the value storage used for conversion.
    ///
    #[inline]
    pub(crate) fn cbadd_value(
        &self,
        ubf: &mut TypedUbf<'_>,
        bfldid: BFLDID,
        buf: *mut c_char,
        len: BFLDLEN,
        usrtype: c_int,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBadd(ubf.as_ubfh(), bfldid, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBadd(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, buf, len, usrtype)
        }
    }

    /// Write an occurrence with native type conversion and return the unmodified C status.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `occ`: Zero-based field occurrence to access.
    /// - `buf`: Readable native value storage matching `usrtype`; pointer fields require the
    ///   address of a pointer variable.
    /// - `len`: Value length in bytes for variable-length input; use the native type’s
    ///   convention for fixed-size values.
    /// - `usrtype`: Native `BFLD_*` type describing the value storage used for conversion.
    ///
    #[inline]
    pub(crate) fn cbchg_value(
        &self,
        ubf: &mut TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut c_char,
        len: BFLDLEN,
        usrtype: c_int,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBchg(ubf.as_ubfh(), bfldid, occ, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBchg(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                occ,
                buf,
                len,
                usrtype,
            )
        }
    }

    /// Read and convert an occurrence into native output storage, returning the C status.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `occ`: Zero-based field occurrence to access.
    /// - `buf`: Writable native destination with at least the input `len` bytes.
    /// - `len`: Destination capacity on input and returned value length on output, in bytes.
    /// - `usrtype`: Native `BFLD_*` type describing the value storage used for conversion.
    ///
    #[inline]
    pub(crate) fn cbget_value(
        &self,
        ubf: &TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut c_char,
        len: &mut BFLDLEN,
        usrtype: c_int,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBget(ubf.as_ubfh(), bfldid, occ, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBget(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                occ,
                buf,
                len,
                usrtype,
            )
        }
    }

    /// Read and convert a field in a borrowed UBF without taking ownership of the child.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `occ`: Zero-based field occurrence to access.
    /// - `buf`: Writable native destination with at least the input `len` bytes.
    /// - `len`: Destination capacity on input and returned value length on output, in bytes.
    /// - `usrtype`: Native `BFLD_*` type describing the value storage used for conversion.
    ///
    #[inline]
    pub(crate) fn cbget_borrowed_ubf_value(
        &self,
        ubf: &BorrowedUbf<'_, '_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut c_char,
        len: &mut BFLDLEN,
        usrtype: c_int,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBget(ubf.as_ubfh(), bfldid, occ, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBget(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                occ,
                buf,
                len,
                usrtype,
            )
        }
    }

    /// Borrow native field storage and report its stored length; return null on failure.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `occ`: Zero-based field occurrence to access.
    /// - `len`: Output slot receiving the native stored field length in bytes.
    ///
    #[inline]
    pub(crate) fn bfind_value(
        &self,
        ubf: &TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        len: &mut BFLDLEN,
    ) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bfind(ubf.as_ubfh(), bfldid, occ, len)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBfind(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ, len)
        }
    }

    /// Advance a cursor that owns its iteration state.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `state`: This iterator’s native cursor, updated after each step.
    /// - `bfldid`: Output slot receiving the next typed field identifier.
    /// - `occ`: Output slot receiving the next zero-based occurrence.
    ///
    /// `Bnext` keeps its position in per-buffer native state, so two live
    /// iterators over one buffer corrupt each other and return `BEINVAL`.
    /// `Bnext2` takes the state explicitly, which lets each iterator hold its
    /// own.
    #[inline]
    pub(crate) fn bnext_value(
        &self,
        ubf: &TypedUbf<'_>,
        state: &mut raw::Bnext_state_t,
        bfldid: &mut BFLDID,
        occ: &mut BFLDOCC,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bnext2(
                state,
                ubf.as_ubfh(),
                bfldid,
                occ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBnext2(
                self.c_ctx_ptr(),
                state,
                ubf.as_ubfh(),
                bfldid,
                occ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        }
    }

    /// Append through the native conversion API using a caller-maintained location cursor.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `buf`: Readable native value storage matching `usrtype`; pointer fields require the
    ///   address of a pointer variable.
    /// - `len`: Value length in bytes for variable-length input; use the native type’s
    ///   convention for fixed-size values.
    /// - `usrtype`: Native `BFLD_*` type describing the value storage used for conversion.
    /// - `loc`: Valid fast-append cursor for the current destination allocation.
    ///
    #[inline]
    pub(crate) fn baddfast_value(
        &self,
        ubf: &mut TypedUbf<'_>,
        bfldid: BFLDID,
        buf: *mut c_char,
        len: BFLDLEN,
        usrtype: c_int,
        loc: &mut BFldLocInfo,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBaddfast(ubf.as_ubfh(), bfldid, buf, len, usrtype, &mut loc.inner)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBaddfast(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                buf,
                len,
                usrtype,
                &mut loc.inner,
            )
        }
    }

    /// Copy an occurrence in its stored native representation without type conversion.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `occ`: Zero-based field occurrence to access.
    /// - `buf`: Writable native destination with at least the input `len` bytes.
    /// - `len`: Destination capacity on input and returned value length on output, in bytes.
    ///
    #[inline]
    pub(crate) fn bget_raw_value(
        &self,
        ubf: &TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut c_char,
        len: &mut BFLDLEN,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bget(ubf.as_ubfh(), bfldid, occ, buf, len)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBget(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ, buf, len)
        }
    }

    /// Compile a native expression string, returning an owned tree pointer or null on failure.
    ///
    /// # Arguments
    ///
    /// - `expr`: Readable NUL-terminated native expression string.
    ///
    #[inline]
    pub(crate) fn bboolco_value(&self, expr: *mut c_char) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bboolco(expr)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBboolco(self.c_ctx_ptr(), expr)
        }
    }

    /// Evaluate a compiled expression with callback context tracking and return the native
    /// boolean status.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `tree`: Compiled expression to evaluate or print.
    ///
    #[inline]
    pub(crate) fn bboolev_value(&self, ubf: &TypedUbf<'_>, tree: &UbfExprTree<'_>) -> c_int {
        let _evaluation = ExprEvaluation::enter(self);
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bboolev(ubf.as_ubfh(), tree.as_ptr())
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBboolev(self.c_ctx_ptr(), ubf.as_ubfh(), tree.as_ptr())
        }
    }

    /// Evaluate a compiled numeric expression with callback context tracking.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `tree`: Compiled expression to evaluate or print.
    ///
    #[inline]
    pub(crate) fn bfloatev_value(&self, ubf: &TypedUbf<'_>, tree: &UbfExprTree<'_>) -> f64 {
        let _evaluation = ExprEvaluation::enter(self);
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bfloatev(ubf.as_ubfh(), tree.as_ptr())
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBfloatev(self.c_ctx_ptr(), ubf.as_ubfh(), tree.as_ptr())
        }
    }

    /// Free a native compiled expression using the matching context API.
    ///
    /// # Arguments
    ///
    /// - `tree`: Owned tree pointer returned by native expression compilation; freed exactly once.
    ///
    #[inline]
    pub(crate) fn btreefree_value(&self, tree: *mut c_char) {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Btreefree(tree)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBtreefree(self.c_ctx_ptr(), tree)
        }
    }

    /// Capture native UBF text output as a UTF-8 string without a trailing NUL.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    pub(crate) fn bfprintcb_value(&self, ubf: &TypedUbf<'_>) -> UbfResult<String> {
        let mut state = OutputState { bytes: Vec::new() };

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bfprintcb(
                ubf.as_ubfh(),
                Some(bfprint_output_callback),
                &mut state as *mut OutputState as *mut c_void,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBfprintcb(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                Some(bfprint_output_callback),
                &mut state as *mut OutputState as *mut c_void,
            )
        };

        if rc != raw::EXSUCCEED as c_int {
            return Err(self.ubf_last_error());
        }

        if state.bytes.last().copied() == Some(0) {
            state.bytes.pop();
        }
        String::from_utf8(state.bytes)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEUNIX, e.to_string()))
    }

    /// Capture the native binary UBF representation in an owned byte vector.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    pub(crate) fn bwritecb_value(&self, ubf: &TypedUbf<'_>) -> UbfResult<Vec<u8>> {
        let mut state = OutputState { bytes: Vec::new() };

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bwritecb(
                ubf.as_ubfh(),
                Some(write_callback),
                &mut state as *mut OutputState as *mut c_void,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBwritecb(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                Some(write_callback),
                &mut state as *mut OutputState as *mut c_void,
            )
        };

        if rc != raw::EXSUCCEED as c_int {
            Err(self.ubf_last_error())
        } else {
            Ok(state.bytes)
        }
    }

    /// Load binary UBF data through a temporary C memory stream.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `dump`: Binary representation produced by native `Bwrite` or this binding’s `bwrite`.
    pub(crate) fn breadcb_value(&self, ubf: &mut TypedUbf<'_>, dump: &[u8]) -> UbfResult<()> {
        let mut data = dump.to_vec();
        let mode = CString::new("rb").expect("static mode has no NUL");
        let file =
            unsafe { libc::fmemopen(data.as_mut_ptr() as *mut c_void, data.len(), mode.as_ptr()) };
        if file.is_null() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEUNIX,
                "failed to open memory stream",
            ));
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bread(ubf.as_ubfh(), file as *mut raw::FILE) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBread(self.c_ctx_ptr(), ubf.as_ubfh(), file as *mut raw::FILE) };

        unsafe {
            libc::fclose(file);
        }

        self.ubf_unit_result(rc)
    }

    /// Read UBF text with the Rust parser, falling back to native parsing when needed.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `text`: UBF text with a field label and value separated by a tab on each line.
    pub(crate) fn bextreadcb_value(&self, ubf: &mut TypedUbf<'_>, text: &str) -> UbfResult<()> {
        if self.bextread_text_rust(ubf, text).is_ok() {
            return Ok(());
        }

        let normalized = self.normalize_bextread_text(text);
        let mut data = CString::new(normalized)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?
            .into_bytes_with_nul();
        let mode = CString::new("r").expect("static mode has no NUL");
        let file =
            unsafe { libc::fmemopen(data.as_mut_ptr() as *mut c_void, data.len(), mode.as_ptr()) };
        if file.is_null() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEUNIX,
                "failed to open memory stream",
            ));
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bextread(ubf.as_ubfh(), file as *mut raw::FILE) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBextread(self.c_ctx_ptr(), ubf.as_ubfh(), file as *mut raw::FILE) };

        unsafe {
            libc::fclose(file);
        }

        self.ubf_unit_result(rc)
    }

    /// Append tab-separated scalar field values, growing the destination as needed.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `text`: UBF text with a field label and value separated by a tab on each line.
    fn bextread_text_rust(&self, ubf: &mut TypedUbf<'_>, text: &str) -> UbfResult<()> {
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let (field, value) = line.split_once('\t').ok_or_else(|| {
                crate::UbfError::new(crate::UbfError::BEINVAL, "missing field/value separator")
            })?;
            let bfldid = self.bextread_field_id(field)?;
            match self.bfldtype(bfldid)? {
                UbfFieldType::Short => ubf.badd(
                    bfldid,
                    value.parse::<i16>().map_err(|e| {
                        crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string())
                    })?,
                    true,
                )?,
                UbfFieldType::Long => ubf.badd(
                    bfldid,
                    value.parse::<i64>().map_err(|e| {
                        crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string())
                    })?,
                    true,
                )?,
                UbfFieldType::Char => {
                    let ch = value.as_bytes().first().copied().unwrap_or_default() as i8;
                    ubf.badd(bfldid, ch, true)?
                }
                UbfFieldType::Float => ubf.badd(
                    bfldid,
                    value.parse::<f32>().map_err(|e| {
                        crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string())
                    })?,
                    true,
                )?,
                UbfFieldType::Double => ubf.badd(
                    bfldid,
                    value.parse::<f64>().map_err(|e| {
                        crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string())
                    })?,
                    true,
                )?,
                UbfFieldType::String => ubf.badd(bfldid, value, true)?,
                UbfFieldType::Carray => ubf.badd(bfldid, value.as_bytes().to_vec(), true)?,
                UbfFieldType::Ptr | UbfFieldType::Ubf | UbfFieldType::View => {
                    return Err(crate::UbfError::new(
                        crate::UbfError::BEINVAL,
                        "BExtRead for ptr/ubf/view fields is not supported by Rust parser",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Resolve a field name or a printed `((BFLDID32)number)` identifier.
    ///
    /// # Arguments
    ///
    /// - `field`: Field name or printed numeric field identifier.
    fn bextread_field_id(&self, field: &str) -> UbfResult<i32> {
        if let Some(id) = field
            .strip_prefix("((BFLDID32)")
            .and_then(|s| s.strip_suffix(')'))
            .and_then(|id| id.parse::<i32>().ok())
        {
            return Ok(id);
        }
        self.bfldid(field)
    }

    /// Replace resolvable numeric field labels with names for native text parsing.
    ///
    /// # Arguments
    ///
    /// - `text`: UBF text with a field label and value separated by a tab on each line.
    fn normalize_bextread_text(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for line in text.lines() {
            if let Some((field, rest)) = line.split_once('\t') {
                let name = field
                    .strip_prefix("((BFLDID32)")
                    .and_then(|s| s.strip_suffix(')'))
                    .and_then(|id| id.parse::<i32>().ok())
                    .and_then(|id| self.bfname(id as BFLDID).ok());
                if let Some(name) = name {
                    out.push_str(&name);
                } else {
                    out.push_str(field);
                }
                out.push('\t');
                out.push_str(rest);
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        out
    }

    /// Return whether two UBF buffers contain the same fields and values.
    ///
    /// # Arguments
    ///
    /// - `ubf1`: First UBF buffer to compare.
    /// - `ubf2`: Second UBF buffer to compare.
    pub fn bcmp(&self, ubf1: &TypedUbf<'_>, ubf2: &TypedUbf<'_>) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bcmp(ubf1.as_ubfh(), ubf2.as_ubfh()) == 0
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBcmp(self.c_ctx_ptr(), ubf1.as_ubfh(), ubf2.as_ubfh()) == 0
        }
    }

    /// Append all fields from `src` into `dst`.
    ///
    /// # Arguments
    ///
    /// - `dst`: Destination UBF to modify; it must already have enough capacity.
    /// - `src`: Source UBF to read; shallow copying rejects pointer fields, including inside
    ///   inline UBFs.
    pub fn bconcat(&self, dst: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        self.reject_pointer_copy(src, "Bconcat")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bconcat(dst.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBconcat(self.c_ctx_ptr(), dst.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Refuse a copying operation whose source holds `BFLD_PTR` fields.
    ///
    /// # Arguments
    ///
    /// - `src`: Source UBF to read; shallow copying rejects pointer fields, including inside
    ///   inline UBFs.
    /// - `what`: Operation name included in a rejected-copy error.
    ///
    /// Every one of these duplicates the stored pointer *addresses* without
    /// duplicating their targets, so the destination ends up referencing
    /// allocations the source still owns and frees. Use `TypedUbf::deep_clone`
    /// to copy the targets into independent allocations.
    fn reject_pointer_copy(&self, src: &TypedUbf<'_>, what: &str) -> UbfResult<()> {
        if self.ubf_has_pointer_fields(src)? {
            return Err(crate::UbfError::new(
                crate::UbfError::BEINVAL,
                format!(
                    "{what} on a buffer containing BFLD_PTR fields would copy the \
                     pointers without their targets, leaving both buffers owning \
                     the same allocations"
                ),
            ));
        }
        Ok(())
    }

    /// Whether `ubf` holds a `BFLD_PTR` field at any depth.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    ///
    /// Wraps `Bhasptr(3)`, which recurses through embedded `BFLD_UBF` fields
    /// because a pointer nested inside one carries the same ownership problem
    /// as a top-level one. It does not follow the pointer targets: finding one
    /// already answers the question.
    ///
    /// UBF stores fields ordered by field id and the id encodes the type in its
    /// high bits, so the pointer occurrences form one contiguous run whose
    /// start the buffer header caches. `Bhasptr` seeks straight to it, which
    /// makes the common pointer-free answer a single step rather than a walk
    /// over the whole buffer.
    pub fn ubf_has_pointer_fields(&self, ubf: &TypedUbf<'_>) -> UbfResult<bool> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bhasptr(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBhasptr(self.c_ctx_ptr(), ubf.as_ubfh()) };

        if rc < 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(rc == raw::EXTRUE as c_int)
        }
    }

    /// Copy the full contents of `src` into `dst`.
    ///
    /// # Arguments
    ///
    /// - `dst`: Destination UBF to modify; it must already have enough capacity.
    /// - `src`: Source UBF to read; shallow copying rejects pointer fields, including inside
    ///   inline UBFs.
    ///
    /// Rejected with `BEINVAL` when `src` contains `BFLD_PTR` fields at any
    /// depth. `Bcpy` copies the stored *addresses*, so both buffers would then
    /// reference the same targets while each believes it owns them: dropping
    /// `src` frees targets that `dst` still points at. Use `TypedUbf::deep_clone`
    /// when an independent copy of the pointer targets is required.
    pub fn bcpy(&self, dst: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        self.reject_pointer_copy(src, "Bcpy")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bcpy(dst.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBcpy(self.c_ctx_ptr(), dst.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Delete one field occurrence and shift later occurrences down.
    ///
    /// Native deletion removes references without freeing pointer targets. Use
    /// [`TypedUbf::bdel_owned`] when removing an occurrence that owns children.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `occ`: Zero-based field occurrence to access.
    pub fn bdel(&self, ubf: &mut TypedUbf<'_>, bfldid: BFLDID, occ: BFLDOCC) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bdel(ubf.as_ubfh(), bfldid, occ) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBdel(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ) };

        self.ubf_unit_result(rc)
    }

    /// Delete all occurrences of a field without freeing any referenced pointer targets.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    pub fn bdelall(&self, ubf: &mut TypedUbf<'_>, bfldid: BFLDID) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bdelall(ubf.as_ubfh(), bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBdelall(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid) };

        self.ubf_unit_result(rc)
    }

    /// Delete all listed fields without freeing any referenced pointer targets.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `fldlist`: Typed field identifiers to select; no terminating zero is required.
    ///
    /// A terminating `0` is appended for the C API; callers should pass field
    /// identifiers only, without a terminator.
    pub fn bdelete(&self, ubf: &mut TypedUbf<'_>, fldlist: &[i32]) -> UbfResult<()> {
        let mut fields: Vec<BFLDID> = fldlist.iter().copied().map(|f| f as BFLDID).collect();
        fields.push(0);

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bdelete(ubf.as_ubfh(), fields.as_mut_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBdelete(self.c_ctx_ptr(), ubf.as_ubfh(), fields.as_mut_ptr()) };

        self.ubf_unit_result(rc)
    }

    /// Query the native index-size compatibility API; current Enduro/X returns zero.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    pub fn bidxused(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bidxused(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBidxused(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Call the native indexing compatibility API; Enduro/X indexes UBF buffers automatically.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `occ`: Legacy index interval; ignored by current Enduro/X.
    pub fn bindex(&self, ubf: &mut TypedUbf<'_>, occ: BFLDOCC) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bindex(ubf.as_ubfh(), occ) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBindex(self.c_ctx_ptr(), ubf.as_ubfh(), occ) };

        self.ubf_unit_result(rc)
    }

    /// Return whether a buffer is a valid UBF buffer.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    pub fn bisubf(&self, ubf: &TypedUbf<'_>) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bisubf(ubf.as_ubfh()) != 0
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBisubf(self.c_ctx_ptr(), ubf.as_ubfh()) != 0
        }
    }

    /// Keep matching field occurrences in `dest`, replacing their values from `src` and
    /// removing unmatched ones.
    ///
    /// # Arguments
    ///
    /// - `dest`: Destination UBF to modify; it must already have enough capacity.
    /// - `src`: Source UBF to read; shallow copying rejects pointer fields, including inside
    ///   inline UBFs.
    pub fn bjoin(&self, dest: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        self.reject_pointer_copy(src, "Bjoin")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bjoin(dest.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBjoin(self.c_ctx_ptr(), dest.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Return the stored length of a field occurrence.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `occ`: Zero-based field occurrence to access.
    pub fn blen(&self, ubf: &TypedUbf<'_>, bfldid: BFLDID, occ: BFLDOCC) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Blen(ubf.as_ubfh(), bfldid, occ) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBlen(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ) };

        self.ubf_count_result(rc)
    }

    /// Return the total number of field occurrences in a UBF buffer.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    pub fn bnum(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bnum(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBnum(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Return the number of occurrences for one field.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    pub fn boccur(&self, ubf: &TypedUbf<'_>, bfldid: BFLDID) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Boccur(ubf.as_ubfh(), bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBoccur(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid) };

        self.ubf_count_result(rc)
    }

    /// Update matching field occurrences from `src`, leaving unmatched destination occurrences
    /// intact.
    ///
    /// # Arguments
    ///
    /// - `dest`: Destination UBF to modify; it must already have enough capacity.
    /// - `src`: Source UBF to read; shallow copying rejects pointer fields, including inside
    ///   inline UBFs.
    pub fn bojoin(&self, dest: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        self.reject_pointer_copy(src, "Bojoin")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bojoin(dest.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBojoin(self.c_ctx_ptr(), dest.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Return whether a field occurrence is present.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `occ`: Zero-based field occurrence to access.
    pub fn bpres(&self, ubf: &TypedUbf<'_>, bfldid: BFLDID, occ: BFLDOCC) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bpres(ubf.as_ubfh(), bfldid, occ) != 0
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBpres(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ) != 0
        }
    }

    /// Return the UBF type for a field id.
    ///
    /// # Arguments
    ///
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    pub fn bfldtype(&self, bfldid: BFLDID) -> UbfResult<UbfFieldType> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bfldtype(bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBfldtype(self.c_ctx_ptr(), bfldid) };

        if rc < 0 {
            return Err(self.ubf_last_error());
        }

        UbfFieldType::from_raw(rc)
            .ok_or_else(|| crate::UbfError::new(crate::UbfError::BEINVAL, "unknown UBF field type"))
    }

    /// Refuse to run Enduro/X's `BFLD_PTR` conversions on `bfldid`.
    ///
    /// # Arguments
    ///
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    /// - `op`: Operation name included in a rejected pointer-conversion error.
    ///
    /// The conversion table pairs `BFLD_PTR` with the scalar types, so reading a
    /// pointer field as an integer hands out the address of a buffer that the
    /// parent still owns and will free, and writing an integer into one installs
    /// an arbitrary address that the parent later passes to `tpfree`
    /// (`ndrx_tpfree_scan_ptrs`, `libatmi/typed_buf.c`). Ownership is only
    /// tracked when the value is a [`crate::UbfValue::Ptr`], so that is the one
    /// form allowed to cross a pointer field.
    pub(crate) fn reject_ptr_conversion(&self, bfldid: BFLDID, op: &str) -> UbfResult<()> {
        if self.bfldtype(bfldid)? == UbfFieldType::Ptr {
            return Err(crate::UbfError::new(
                crate::UbfError::BTYPERR,
                format!(
                    "{op}: field id {bfldid} is BFLD_PTR; write it with an owned buffer \
                     and read it back with bget_ptr or bextract_ptr"
                ),
            ));
        }
        Ok(())
    }

    /// Resolve a field name to its typed field id.
    ///
    /// # Arguments
    ///
    /// - `field_name`: UBF field-table name, without embedded NUL bytes.
    pub fn bfldid(&self, field_name: &str) -> UbfResult<i32> {
        let name = CString::new(field_name)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?;

        #[cfg(not(feature = "ctx-send"))]
        let mut rc = unsafe { raw::Bfldid(name.as_ptr() as *mut c_char) };

        #[cfg(feature = "ctx-send")]
        let mut rc = unsafe { raw::OBfldid(self.c_ctx_ptr(), name.as_ptr() as *mut c_char) };

        if rc <= 0 {
            #[cfg(not(feature = "ctx-send"))]
            unsafe {
                raw::Bflddbload();
                rc = raw::Bflddbid(name.as_ptr() as *mut c_char);
            }

            #[cfg(feature = "ctx-send")]
            unsafe {
                raw::OBflddbload(self.c_ctx_ptr());
                rc = raw::OBflddbid(self.c_ctx_ptr(), name.as_ptr() as *mut c_char);
            }
        }

        if rc <= 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Resolve a typed field id to its field name.
    ///
    /// # Arguments
    ///
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    pub fn bfname(&self, bfldid: BFLDID) -> UbfResult<String> {
        #[cfg(not(feature = "ctx-send"))]
        let mut ptr = unsafe { raw::Bfname(bfldid) };

        #[cfg(feature = "ctx-send")]
        let mut ptr = unsafe { raw::OBfname(self.c_ctx_ptr(), bfldid) };

        if ptr.is_null() {
            #[cfg(not(feature = "ctx-send"))]
            unsafe {
                raw::Bflddbload();
                ptr = raw::Bflddbname(bfldid);
            }

            #[cfg(feature = "ctx-send")]
            unsafe {
                raw::OBflddbload(self.c_ctx_ptr());
                ptr = raw::OBflddbname(self.c_ctx_ptr(), bfldid);
            }
        }

        if ptr.is_null() {
            Err(self.ubf_last_error())
        } else {
            Ok(unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned())
        }
    }

    /// Return the untyped field number portion of a typed field id.
    ///
    /// # Arguments
    ///
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    pub fn bfldno(&self, bfldid: BFLDID) -> i32 {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bfldno(bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBfldno(self.c_ctx_ptr(), bfldid) };

        rc as i32
    }

    /// Return the Enduro/X textual field type descriptor.
    ///
    /// # Arguments
    ///
    /// - `bfldid`: Typed field identifier, including the native field-type bits.
    pub fn btype(&self, bfldid: BFLDID) -> UbfResult<String> {
        #[cfg(not(feature = "ctx-send"))]
        let ptr = unsafe { raw::Btype(bfldid) };

        #[cfg(feature = "ctx-send")]
        let ptr = unsafe { raw::OBtype(self.c_ctx_ptr(), bfldid) };

        if ptr.is_null() {
            Err(self.ubf_last_error())
        } else {
            Ok(unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned())
        }
    }

    /// Reinitialize a UBF buffer with a given UBF length.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `len`: UBF capacity to record in bytes; must not exceed the actual allocation.
    ///
    /// Fails with `BEINVAL` if `len` exceeds the allocation. `Binit` formats the
    /// buffer header for the length it is given, so a value larger than the
    /// allocation makes every later UBF operation address memory past the end
    /// of it.
    pub fn binit(&self, ubf: &mut TypedUbf<'_>, len: usize) -> UbfResult<()> {
        let cap = ubf.tptypes().map(|info| info.size).unwrap_or(0);
        if len > cap {
            return Err(crate::UbfError::new(
                crate::UbfError::BEINVAL,
                format!("Binit length {len} exceeds the {cap} byte buffer allocation"),
            ));
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Binit(ubf.as_ubfh(), len as raw::BFLDLEN) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBinit(self.c_ctx_ptr(), ubf.as_ubfh(), len as raw::BFLDLEN) };

        self.ubf_unit_result(rc)
    }

    /// Load the configured native UBF field database for dynamic name/identifier lookup.
    pub fn bflddbload(&self) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bflddbload() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBflddbload(self.c_ctx_ptr()) };

        self.ubf_unit_result(rc)
    }

    /// Compile a UBF boolean expression.
    ///
    /// # Arguments
    ///
    /// - `expr`: UBF boolean expression text to compile.
    pub fn bboolco(&self, expr: &str) -> UbfResult<UbfExprTree<'_>> {
        let expr = CString::new(expr)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?;
        let _ = self.bflddbload();
        let ptr = self.bboolco_value(expr.as_ptr() as *mut c_char);

        if ptr.is_null() {
            Err(self.ubf_last_error())
        } else {
            Ok(UbfExprTree { ptr, ctx: self })
        }
    }

    /// Explicitly free a compiled UBF boolean expression tree.
    ///
    /// # Arguments
    ///
    /// - `tree`: Owned expression tree to consume and release.
    pub fn btreefree(&self, mut tree: UbfExprTree<'_>) {
        tree.free();
    }

    /// Print a compiled boolean expression tree to a string.
    ///
    /// # Arguments
    ///
    /// - `tree`: Compiled expression to evaluate or print.
    ///
    /// Wraps the C `Bboolpr`/`OBboolpr` `FILE*` API by capturing its output via
    /// an in-memory stream from `open_memstream(3)`.
    pub fn bboolpr(&self, tree: &UbfExprTree<'_>) -> UbfResult<String> {
        let mut buf_ptr: *mut c_char = std::ptr::null_mut();
        let mut buf_size: libc::size_t = 0;
        let file = unsafe { libc::open_memstream(&mut buf_ptr, &mut buf_size) };
        if file.is_null() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEUNIX,
                "failed to open memory stream",
            ));
        }

        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bboolpr(tree.as_ptr(), file as *mut raw::FILE);
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBboolpr(self.c_ctx_ptr(), tree.as_ptr(), file as *mut raw::FILE);
        }

        unsafe {
            libc::fclose(file);
        }

        if buf_ptr.is_null() {
            return Ok(String::new());
        }

        let bytes = unsafe { std::slice::from_raw_parts(buf_ptr as *const u8, buf_size) }.to_vec();
        unsafe {
            libc::free(buf_ptr as *mut c_void);
        }

        String::from_utf8(bytes)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEUNIX, e.to_string()))
    }

    /// Register a Rust callback for UBF boolean expression evaluation.
    ///
    /// # Arguments
    ///
    /// - `funcname`: Registered function name, without embedded NUL bytes.
    /// - `callback`: Function receiving the evaluated UBF and registered name, and returning
    ///   the expression value.
    ///
    /// Registration is process-wide and independent of this context's lifetime.
    /// The callback borrows the context of the buffer being evaluated, on the
    /// evaluating thread. The same callback can run concurrently on several
    /// threads, so it must synchronize any shared application state.
    pub fn bboolsetcbf(&self, funcname: &str, callback: UbfExprCallback) -> UbfResult<()> {
        if funcname.is_empty() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEINVAL,
                "function name is empty",
            ));
        }
        let c_funcname = CString::new(funcname)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bboolsetcbf(
                c_funcname.as_ptr() as *mut c_char,
                Some(expr_callback_proxy),
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBboolsetcbf(
                self.c_ctx_ptr(),
                c_funcname.as_ptr() as *mut c_char,
                Some(expr_callback_proxy),
            )
        };

        if rc != raw::EXSUCCEED as c_int {
            Err(self.ubf_last_error())
        } else {
            expr_callbacks()
                .lock()
                .expect("UBF expression callback registry poisoned")
                .insert(funcname.to_string(), callback);
            Ok(())
        }
    }

    /// Register a Rust callback with one string argument for boolean evaluation.
    ///
    /// # Arguments
    ///
    /// - `funcname`: Registered function name, without embedded NUL bytes.
    /// - `callback`: Function receiving the evaluated UBF, registered name, and
    ///   expression-supplied string argument.
    ///
    /// Like [`Self::bboolsetcbf`], registration is process-wide. The callback
    /// borrows the evaluating buffer's context, regardless of which context or
    /// thread registered it, and may run concurrently on different threads.
    pub fn bboolsetcbf2(&self, funcname: &str, callback: UbfExprCallback2) -> UbfResult<()> {
        if funcname.is_empty() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEINVAL,
                "function name is empty",
            ));
        }
        let c_funcname = CString::new(funcname)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bboolsetcbf2(
                c_funcname.as_ptr() as *mut c_char,
                Some(expr_callback_proxy2),
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBboolsetcbf2(
                self.c_ctx_ptr(),
                c_funcname.as_ptr() as *mut c_char,
                Some(expr_callback_proxy2),
            )
        };

        if rc != raw::EXSUCCEED as c_int {
            Err(self.ubf_last_error())
        } else {
            expr_callbacks2()
                .lock()
                .expect("UBF expression callback registry poisoned")
                .insert(funcname.to_string(), callback);
            Ok(())
        }
    }

    /// Project a UBF buffer in place to the fields listed in `fldlist`.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    /// - `fldlist`: Typed field identifiers to select; no terminating zero is required.
    ///
    /// A terminating `0` is appended for the C API; callers should pass field
    /// identifiers only, without a terminator.
    pub fn bproj(&self, ubf: &mut TypedUbf<'_>, fldlist: &[i32]) -> UbfResult<()> {
        let mut fields: Vec<BFLDID> = fldlist.iter().copied().map(|f| f as BFLDID).collect();
        fields.push(0);

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bproj(ubf.as_ubfh(), fields.as_mut_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBproj(self.c_ctx_ptr(), ubf.as_ubfh(), fields.as_mut_ptr()) };

        self.ubf_unit_result(rc)
    }

    /// Copy a projection of `src` into `dst`.
    ///
    /// # Arguments
    ///
    /// - `dst`: Destination UBF to modify; it must already have enough capacity.
    /// - `src`: Source UBF to read; shallow copying rejects pointer fields, including inside
    ///   inline UBFs.
    /// - `fldlist`: Typed field identifiers to select; no terminating zero is required.
    pub fn bprojcpy(
        &self,
        dst: &mut TypedUbf<'_>,
        src: &TypedUbf<'_>,
        fldlist: &[i32],
    ) -> UbfResult<()> {
        self.reject_pointer_copy(src, "Bprojcpy")?;
        let mut fields: Vec<BFLDID> = fldlist.iter().copied().map(|f| f as BFLDID).collect();
        fields.push(0);

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bprojcpy(dst.as_ubfh(), src.as_ubfh(), fields.as_mut_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBprojcpy(
                self.c_ctx_ptr(),
                dst.as_ubfh(),
                src.as_ubfh(),
                fields.as_mut_ptr(),
            )
        };

        self.ubf_unit_result(rc)
    }

    /// Return the allocated size of a UBF buffer in bytes.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    pub fn bsizeof(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bsizeof(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBsizeof(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Test whether `ubf2` is contained in `ubf1`, comparing fields, occurrences, and values.
    ///
    /// # Arguments
    ///
    /// - `ubf1`: Containing buffer to search.
    /// - `ubf2`: Candidate subset whose fields and values are sought in `ubf1`.
    ///
    /// This wrapper maps any nonzero native result to `true`, including a native error.
    pub fn bsubset(&self, ubf1: &TypedUbf<'_>, ubf2: &TypedUbf<'_>) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bsubset(ubf1.as_ubfh(), ubf2.as_ubfh()) != 0
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBsubset(self.c_ctx_ptr(), ubf1.as_ubfh(), ubf2.as_ubfh()) != 0
        }
    }

    /// Call the native unindex compatibility API; current Enduro/X returns zero without
    /// changing the buffer.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF whose fields or native header may be changed.
    pub fn bunindex(&self, ubf: &mut TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bunindex(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBunindex(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Return the unused byte count in a UBF buffer.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    pub fn bunused(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bunused(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBunused(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Replace matching field occurrences in `dst` from `src`, adding source occurrences that
    /// are missing.
    ///
    /// # Arguments
    ///
    /// - `dst`: Destination UBF to modify; it must already have enough capacity.
    /// - `src`: Source UBF to read; shallow copying rejects pointer fields, including inside
    ///   inline UBFs.
    pub fn bupdate(&self, dst: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        self.reject_pointer_copy(src, "Bupdate")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bupdate(dst.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBupdate(self.c_ctx_ptr(), dst.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Return the used byte count in a UBF buffer.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer to inspect.
    pub fn bused(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bused(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBused(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Return a typed field id from a field type and field number.
    ///
    /// # Arguments
    ///
    /// - `field_type`: Rust UBF field kind to encode in the identifier.
    /// - `field_no`: Untyped field number to combine with the field kind.
    pub fn bmkfldid_typed(&self, field_type: UbfFieldType, field_no: i32) -> i32 {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bmkfldid(field_type.as_raw(), field_no as BFLDID) };

        #[cfg(feature = "ctx-send")]
        let rc =
            unsafe { raw::OBmkfldid(self.c_ctx_ptr(), field_type.as_raw(), field_no as BFLDID) };

        rc as i32
    }

    /// Return a typed field id from a raw Enduro/X field type and field number.
    ///
    /// # Arguments
    ///
    /// - `field_type`: Native `BFLD_*` numeric type code to encode in the identifier.
    /// - `field_no`: Untyped field number to combine with the field kind.
    pub fn bmkfldid(&self, field_type: i32, field_no: i32) -> UbfResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bmkfldid(field_type as c_int, field_no as BFLDID) };

        #[cfg(feature = "ctx-send")]
        let rc =
            unsafe { raw::OBmkfldid(self.c_ctx_ptr(), field_type as c_int, field_no as BFLDID) };

        if rc < 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(rc as i32)
        }
    }
}
