//! Service registration, reply ownership transfer, server lifecycle hooks, and poll callbacks.
use crate::{raw, AtmiCtx, AtmiError, AtmiResult, TpSvcInfo, TypedBuffer, TypedUbf};
use core::ffi::{c_char, c_int, c_long};
use std::ffi::{CStr, CString};
use std::{
    cell::RefCell,
    collections::HashMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Mutex, OnceLock},
};

/// Low-level C-compatible service callback used by Enduro/X registration APIs.
type ServiceCallback = unsafe extern "C" fn(*mut raw::TPSVCINFO);
type PollerCallback = unsafe extern "C" fn(c_int, u32, *mut ::std::os::raw::c_void) -> c_int;
type PeriodCallback = unsafe extern "C" fn() -> c_int;
type BeforePollCallback = unsafe extern "C" fn() -> c_int;
type ServerInitHook = unsafe extern "C" fn(c_int, *mut *mut c_char) -> c_int;
type ServerDoneHook = unsafe extern "C" fn();

/// High-level server init callback.
///
/// Receives the server's command-line arguments as Enduro/X delivers them
/// to the C-level `tpsvrinit(int argc, char **argv)` — i.e. only the user
/// arguments after the framework's own option processing (typically what
/// follows the `--` separator in `CLOPT`). The first element is the program
/// name when present.
///
/// Returning `Err(...)` aborts server startup.
pub type RustServerInitHook = fn(&AtmiCtx, &[String]) -> AtmiResult<()>;

/// High-level server shutdown callback receiving the still-active main context.
pub type RustServerDoneHook = fn(&AtmiCtx);

/// Per-worker-thread init callback, mirroring C `tpsvrthrinit`.
///
/// Runs once on each libatmisrv dispatch thread, after Enduro/X has opened that
/// worker's ATMI session and before it takes any request, so the supplied
/// context is that worker's own. Use it for per-thread resources -- a database
/// handle, a thread-local cache -- which have nowhere else to live: the context
/// a service handler receives is built per dispatch, not per thread.
///
/// Only called when dispatch threading is active (`maxdispatchthreads > 1`).
/// Returning `Err(...)` aborts that worker's startup.
pub type RustServerThreadInitHook = fn(&AtmiCtx, &[String]) -> AtmiResult<()>;

/// Per-worker-thread shutdown callback, mirroring C `tpsvrthrdone`.
///
/// Runs on the worker thread before Enduro/X terminates its ATMI session, so
/// the context is still usable.
pub type RustServerThreadDoneHook = fn(&AtmiCtx);

/// The four C server lifecycle hooks, as one value.
///
/// Only `tpsvrinit` is mandatory; the rest default to absent. Enduro/X's own
/// `tpsvrthrinit`/`tpsvrthrdone` defaults still run either way -- they perform
/// the worker's `tx_open()`/`tx_close()` -- and a Rust thread hook runs in
/// addition to them, not instead.
#[derive(Debug, Clone, Copy)]
pub struct ServerHooks {
    init: RustServerInitHook,
    done: Option<RustServerDoneHook>,
    thread_init: Option<RustServerThreadInitHook>,
    thread_done: Option<RustServerThreadDoneHook>,
}

/// Builder methods for main-thread and worker-thread lifecycle hooks.
impl ServerHooks {
    /// Start from the mandatory `tpsvrinit` hook.
    ///
    /// # Arguments
    ///
    /// - `init`: Main initialization function, receiving the active context and processed
    ///   server arguments.
    pub fn new(init: RustServerInitHook) -> Self {
        Self {
            init,
            done: None,
            thread_init: None,
            thread_done: None,
        }
    }

    /// Set the `tpsvrdone` hook.
    ///
    /// # Arguments
    ///
    /// - `done`: Main shutdown function, called while its context remains usable.
    pub fn done(mut self, done: RustServerDoneHook) -> Self {
        self.done = Some(done);
        self
    }

    /// Set the per-worker-thread `tpsvrthrinit` hook.
    ///
    /// # Arguments
    ///
    /// - `thread_init`: Worker initialization function; receives that worker’s context and
    ///   server arguments.
    pub fn thread_init(mut self, thread_init: RustServerThreadInitHook) -> Self {
        self.thread_init = Some(thread_init);
        self
    }

    /// Set the per-worker-thread `tpsvrthrdone` hook.
    ///
    /// # Arguments
    ///
    /// - `thread_done`: Worker cleanup function; runs before Enduro/X terminates the worker
    ///   context.
    pub fn thread_done(mut self, thread_done: RustServerThreadDoneHook) -> Self {
        self.thread_done = Some(thread_done);
        self
    }
}

/// High-level service callback used by [`AtmiCtx::tpadvertise`].
///
/// When Enduro/X dispatch threading is configured, the same function may be
/// invoked concurrently on several libatmisrv worker threads. Any application
/// state accessed by the handler must therefore be thread-safe.
///
/// Arguments are the invoking ATMI context and mutable request metadata. Take the
/// request buffer from the metadata before returning or forwarding it.
pub type RustServiceCallback = for<'ctx> fn(&'ctx AtmiCtx, &mut TpSvcInfo<'ctx>);

/// Event delivered to a Rust poller callback registered with
/// [`AtmiCtx::tpext_addpollerfd`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollerEvent {
    /// Registered descriptor reporting readiness.
    pub fd: i32,
    /// Native poll-event bitmask reported for this descriptor.
    pub events: u32,
    /// Application value supplied when this descriptor was registered.
    pub user_data: usize,
}

/// High-level poller callback used by [`AtmiCtx::tpext_addpollerfd`].
/// The context is the owning main server context because extension events are
/// always dispatched by the main poll loop, never by service workers.
/// Receives that context and a `PollerEvent`; return `0` for success or `-1` for failure.
pub type RustPollerCallback = for<'ctx> fn(&'ctx AtmiCtx, PollerEvent) -> i32;

/// High-level periodic callback used by [`AtmiCtx::tpext_addperiodcb`].
/// The callback receives the owning main server context.
/// Return `0` for success or `-1` to report failure to the native event loop.
pub type RustPeriodCallback = for<'ctx> fn(&'ctx AtmiCtx) -> i32;

/// High-level before-poll callback used by [`AtmiCtx::tpext_addb4pollcb`].
/// The callback receives the owning main server context.
/// Return `0` for success or `-1` to report failure to the native event loop.
pub type RustBeforePollCallback = for<'ctx> fn(&'ctx AtmiCtx) -> i32;

/// Service return status for [`AtmiCtx::tpreturn`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpReturnStatus {
    Success,
    Fail,
}

/// Conversion of Rust service outcomes to native status values.
impl TpReturnStatus {
    /// Convert the service outcome to native `TPSUCCESS` or `TPFAIL`.
    #[inline]
    fn to_raw(self) -> c_int {
        match self {
            TpReturnStatus::Success => raw::TPSUCCESS as c_int,
            TpReturnStatus::Fail => raw::TPFAIL as c_int,
        }
    }
}

#[derive(Default)]
struct ServerRuntime {
    ctx_addr: usize,
    main_thread_id: Option<std::thread::ThreadId>,
    init_hook: Option<RustServerInitHook>,
    done_hook: Option<RustServerDoneHook>,
    thread_init_hook: Option<RustServerThreadInitHook>,
    thread_done_hook: Option<RustServerThreadDoneHook>,
    init_error: Option<AtmiError>,
    /// First worker-thread init failure, kept so a startup abort is
    /// diagnosable rather than surfacing as a bare EXFAIL.
    thread_init_error: Option<AtmiError>,
    services: HashMap<String, RustServiceCallback>,
}

/// Reset of process-wide service and lifecycle registrations.
impl ServerRuntime {
    /// Clear the active context, lifecycle hooks, saved failures, and service registry.
    fn reset(&mut self) {
        self.ctx_addr = 0;
        self.main_thread_id = None;
        self.init_hook = None;
        self.done_hook = None;
        self.thread_init_hook = None;
        self.thread_done_hook = None;
        self.init_error = None;
        self.thread_init_error = None;
        self.services.clear();
    }
}

static SERVER_RUNTIME: OnceLock<Mutex<ServerRuntime>> = OnceLock::new();

#[derive(Default)]
struct ExtensionRuntime {
    pollers: HashMap<i32, (RustPollerCallback, usize)>,
    period_cb: Option<RustPeriodCallback>,
    before_poll_cb: Option<RustBeforePollCallback>,
}

static EXTENSION_RUNTIME: OnceLock<Mutex<ExtensionRuntime>> = OnceLock::new();

/// Return the lazily initialized server-state mutex.
#[inline]
fn server_runtime() -> &'static Mutex<ServerRuntime> {
    SERVER_RUNTIME.get_or_init(|| Mutex::new(ServerRuntime::default()))
}

/// Return the lazily initialized poll-extension registry mutex.
#[inline]
fn extension_runtime() -> &'static Mutex<ExtensionRuntime> {
    EXTENSION_RUNTIME.get_or_init(|| Mutex::new(ExtensionRuntime::default()))
}

/// Create the error reported when a panic has poisoned server-state locking.
#[inline]
fn runtime_lock_err() -> AtmiError {
    AtmiError::new(raw::TPESYSTEM, "server runtime state is poisoned")
}

struct ServerRuntimeGuard;

/// Clear the active server state when the server runner exits.
impl Drop for ServerRuntimeGuard {
    /// Clear the active server state when the server runner exits.
    fn drop(&mut self) {
        if let Ok(mut rt) = server_runtime().lock() {
            rt.reset();
        }
    }
}

struct ServerThreadModeGuard {
    previous: c_int,
    previous_thread_init: Option<ServerInitHook>,
    previous_thread_done: Option<ServerDoneHook>,
}

/// Temporary installation of native dispatch-thread settings and Rust worker hooks.
impl ServerThreadModeGuard {
    /// Install Rust worker hooks and enable native dispatch-thread support until this guard is
    /// dropped.
    ///
    /// # Safety
    ///
    /// No other server runner may mutate native dispatch globals while this guard is alive.
    unsafe fn enable() -> Self {
        let previous = raw::_tmbuilt_with_thread_option;
        let previous_thread_init = raw::ndrx_G_tpsvrthrinit;
        let previous_thread_done = raw::ndrx_G_tpsvrthrdone;
        raw::_tmbuilt_with_thread_option = 1;
        // Always our own trampolines: they chain to Enduro/X's defaults for
        // tx_open()/tx_close() and additionally dispatch any Rust thread hook.
        raw::ndrx_G_tpsvrthrinit = Some(rust_thread_init);
        raw::ndrx_G_tpsvrthrdone = Some(rust_thread_done);
        Self {
            previous,
            previous_thread_init,
            previous_thread_done,
        }
    }
}

/// Restore the native thread mode and worker hooks saved by this guard.
impl Drop for ServerThreadModeGuard {
    /// Restore the native thread mode and worker hooks saved by this guard.
    fn drop(&mut self) {
        unsafe {
            raw::_tmbuilt_with_thread_option = self.previous;
            raw::ndrx_G_tpsvrthrinit = self.previous_thread_init;
            raw::ndrx_G_tpsvrthrdone = self.previous_thread_done;
        }
    }
}

/// What a service handler asked Enduro/X to do with its reply buffer.
enum ReplyKind {
    /// `tpreturn(rval, rcode, ..)`.
    Return { rval: c_int, rcode: c_long },
    /// `tpforward(svc, ..)`.
    Forward { svc: CString },
}

/// A reply whose buffer ownership has already left Rust.
struct ServiceReply {
    kind: ReplyKind,
    data: *mut c_char,
    len: c_long,
    flags: c_long,
}

/// Reply bookkeeping for the service call being dispatched on this thread.
///
/// Enduro/X registers the request as the call's auto-buffer and frees it inside
/// `tpreturn`/`tpforward` whenever the reply is a different buffer
/// (`libatmisrv/tpreturn.c`). A handler that keeps the request and replies
/// with a new buffer would then hold freed memory: reading it is a
/// use-after-free, and dropping it frees the address again, which under
/// dispatch threads may by then belong to another worker's buffer.
///
/// The public C API offers no way to detach the auto-buffer, so the native
/// reply is only issued once that cannot happen: at once when the reply is the
/// request itself or the request was already freed, otherwise after the
/// handler has returned and dropped everything it held.
#[derive(Default)]
struct DispatchFrame {
    /// The request wrapper was dropped, which ran `tpfree` and cleared the
    /// native auto-buffer. Stays `false` when ownership left Rust another way
    /// (for example into a `BFLD_PTR` field), where the auto-buffer is still
    /// armed.
    request_released: bool,
    /// A reply was issued or parked; any further one is discarded.
    replied: bool,
    /// Reply held back until the handler returns.
    pending: Option<ServiceReply>,
}

thread_local! {
    static DISPATCH_FRAME: RefCell<Option<DispatchFrame>> = const { RefCell::new(None) };
}

/// Record that the dispatched request buffer was freed through its Rust wrapper.
pub(crate) fn service_request_released() {
    // Ignore a destroyed thread-local: no dispatch is active while a thread exits.
    let _ = DISPATCH_FRAME.try_with(|slot| {
        if let Some(frame) = slot.borrow_mut().as_mut() {
            frame.request_released = true;
        }
    });
}

/// Return the original native request buffer with a failure status.
///
/// # Arguments
///
/// - `svc_ptr`: Live native request metadata; its data allocation must remain available for
///   reply ownership transfer.
///
/// # Safety
///
/// The request must be live on the current service context, with its buffer available for
/// transfer. Native returns must not long-jump across Rust frames.
unsafe fn fail_current_service(svc_ptr: *mut raw::TPSVCINFO) {
    let data = (*svc_ptr).data;
    let len = (*svc_ptr).len.max(0) as c_long;
    raw::tpreturn(TpReturnStatus::Fail.to_raw(), 0, data, len, 0);
}

/// Invoke the registered Rust service with its active context and convert panics to failed replies.
///
/// # Arguments
///
/// - `svc_ptr`: Live native request metadata; its data allocation must remain available for
///   reply ownership transfer.
///
/// # Safety
///
/// Enduro/X must invoke this with live request metadata on an initialized dispatch thread. The
/// runner must remain active and use `ATMI_SRVLIB_NOLONGJUMP`.
unsafe extern "C" fn rust_service_dispatch(svc_ptr: *mut raw::TPSVCINFO) {
    if svc_ptr.is_null() {
        return;
    }

    let current_thread = std::thread::current().id();
    let (ctx_addr, is_main_thread) = match server_runtime().lock() {
        Ok(rt) => (
            rt.ctx_addr,
            rt.main_thread_id
                .as_ref()
                .is_some_and(|thread_id| *thread_id == current_thread),
        ),
        Err(_) => {
            fail_current_service(svc_ptr);
            return;
        }
    };
    if ctx_addr == 0 {
        fail_current_service(svc_ptr);
        return;
    }

    // Single-threaded servers dispatch on the tp_run thread, where the owning
    // context is already correct. Threaded servers dispatch on initialized
    // libatmisrv workers and need a callback-scoped worker context instead.
    let worker_ctx = if is_main_thread {
        None
    } else {
        match AtmiCtx::borrow_current_context() {
            Ok(ctx) => Some(ctx),
            Err(_) => {
                fail_current_service(svc_ptr);
                return;
            }
        }
    };
    let ctx = worker_ctx
        .as_ref()
        .unwrap_or_else(|| &*(ctx_addr as *const AtmiCtx));
    // Open the reply bookkeeping before the request wrapper exists. A null
    // request has no auto-buffer to protect.
    let outer_frame = DISPATCH_FRAME.with(|slot| {
        slot.replace(Some(DispatchFrame {
            request_released: (*svc_ptr).data.is_null(),
            ..DispatchFrame::default()
        }))
    });
    let mut svc = TpSvcInfo::from_raw(ctx, svc_ptr);

    let key = if svc.fname().is_empty() {
        svc.name().to_owned()
    } else {
        svc.fname().to_owned()
    };

    let cb = match server_runtime().lock() {
        Ok(rt) => rt
            .services
            .get(&key)
            .copied()
            .or_else(|| rt.services.get(svc.name()).copied()),
        Err(_) => None,
    };

    let panicked = match cb {
        Some(handler) => catch_unwind(AssertUnwindSafe(|| handler(ctx, &mut svc))).is_err(),
        None => false,
    };

    // The handler has returned or unwound, so nothing it held is alive any
    // more: a native reply can no longer free a buffer Rust still reaches.
    let untouched = svc.take_data();
    let frame = DISPATCH_FRAME
        .with(|slot| slot.replace(outer_frame))
        .unwrap_or_default();

    if let Some(reply) = frame.pending {
        // Free an untaken request first, which also clears the auto-buffer.
        drop(untouched);
        ctx.reply_value(reply);
    } else if !frame.replied && (panicked || cb.is_none()) {
        // No handler, or it panicked before replying. Fail the call with the
        // request if it is still here, otherwise with a fresh buffer.
        let err_ptr = match untouched {
            Some(buf) => buf.into_raw(),
            None => ctx
                .tpalloc_ubf(256)
                .map(|u| u.into_inner().into_raw())
                .unwrap_or(std::ptr::null_mut()),
        };
        ctx.reply_value(ServiceReply {
            kind: ReplyKind::Return {
                rval: TpReturnStatus::Fail.to_raw(),
                rcode: 0,
            },
            data: err_ptr,
            len: 0,
            flags: 0,
        });
    }
}

/// Deliver a native descriptor event to its Rust callback on the main server context.
///
/// # Arguments
///
/// - `fd`: File descriptor to register, remove, or dispatch; ownership stays with the caller.
/// - `events`: Native poll-event bitmask selecting or reporting descriptor readiness.
/// - `_ptr1`: Unused native userdata pointer; Rust looks up userdata by descriptor.
///
/// # Safety
///
/// Invoke only from Enduro/X’s main poll loop while the registered server context is alive.
unsafe extern "C" fn rust_poller_dispatch(
    fd: c_int,
    events: u32,
    _ptr1: *mut ::std::os::raw::c_void,
) -> c_int {
    let ctx = match main_extension_context() {
        Some(ctx) => ctx,
        None => return -1,
    };
    let (cb, user_data) = match extension_runtime().lock() {
        Ok(rt) => match rt.pollers.get(&(fd as i32)).copied() {
            Some(v) => v,
            None => return 0,
        },
        Err(_) => return -1,
    };

    catch_unwind(AssertUnwindSafe(|| {
        cb(
            ctx,
            PollerEvent {
                fd: fd as i32,
                events,
                user_data,
            },
        )
    }))
    .unwrap_or(-1)
}

/// Invoke the registered periodic callback, converting a panic to native failure.
///
/// # Safety
///
/// Invoke only from Enduro/X’s main poll loop while the registered server context is alive.
unsafe extern "C" fn rust_period_dispatch() -> c_int {
    let ctx = match main_extension_context() {
        Some(ctx) => ctx,
        None => return -1,
    };
    let cb = match extension_runtime().lock() {
        Ok(rt) => rt.period_cb,
        Err(_) => return -1,
    };

    match cb {
        Some(cb) => catch_unwind(AssertUnwindSafe(|| cb(ctx))).unwrap_or(-1),
        None => 0,
    }
}

/// Invoke the callback that runs before polling, converting a panic to native failure.
///
/// # Safety
///
/// Invoke only from Enduro/X’s main poll loop while the registered server context is alive.
unsafe extern "C" fn rust_before_poll_dispatch() -> c_int {
    let ctx = match main_extension_context() {
        Some(ctx) => ctx,
        None => return -1,
    };
    let cb = match extension_runtime().lock() {
        Ok(rt) => rt.before_poll_cb,
        Err(_) => return -1,
    };

    match cb {
        Some(cb) => catch_unwind(AssertUnwindSafe(|| cb(ctx))).unwrap_or(-1),
        None => 0,
    }
}

/// Reject a poll-extension mutation issued from anywhere but the main thread.
///
/// # Arguments
///
/// - `what`: Operation name included when rejecting a call from a dispatch worker.
///
/// Enduro/X keeps these in one unsynchronised global list that the main
/// dispatch thread walks on every poll iteration, so mutating it from a worker
/// corrupts the poll set and wedges the server. That is a documented
/// constraint, but documentation cannot stop a safe method being called from a
/// service handler under `maxdispatchthreads > 1` -- so it is checked.
///
/// Only enforced while a server is actually running: outside `tp_run` there is
/// no main thread to compare against and no poll loop to race.
fn require_main_thread(what: &str) -> AtmiResult<()> {
    let current = std::thread::current().id();
    let main = match server_runtime().lock() {
        Ok(rt) => rt.main_thread_id,
        Err(_) => return Err(runtime_lock_err()),
    };
    match main {
        Some(main) if main != current => Err(AtmiError::new(
            raw::TPEPROTO,
            format!(
                "{what} must be called from the main server thread; calling it                  from a dispatch worker races Enduro/X's global poll-extension                  list and can wedge the server"
            ),
        )),
        _ => Ok(()),
    }
}

/// Resolve the `tp_run` context for an extension callback, or `None` when it is
/// unavailable or the caller is not the main poll-loop thread.
///
/// # Safety
///
/// The returned `'static` lifetime is wider than the context actually lives:
/// `ctx_addr` points at a stack local owned by [`AtmiCtx::tp_run`]. Two
/// invariants keep it from escaping. `ServerRuntimeGuard` clears `ctx_addr`
/// before `tp_run` returns, so a stale address is never resolved, and the
/// extension callback types are higher-ranked (`for<'ctx> fn(&'ctx AtmiCtx,
/// ..)`), so a callback cannot store the reference it receives. Callers must
/// therefore only narrow this lifetime into a callback invocation and never
/// retain the reference past that call.
unsafe fn main_extension_context() -> Option<&'static AtmiCtx> {
    let current_thread = std::thread::current().id();
    let ctx_addr = match server_runtime().lock() {
        Ok(rt)
            if rt.ctx_addr != 0
                && rt
                    .main_thread_id
                    .as_ref()
                    .is_some_and(|thread_id| *thread_id == current_thread) =>
        {
            rt.ctx_addr
        }
        _ => return None,
    };
    Some(&*(ctx_addr as *const AtmiCtx))
}

/// Collect `argc`/`argv` as Enduro/X hands them to a C hook.
///
/// # Arguments
///
/// - `argc`: Number of entries in the native argument array.
/// - `argv`: Readable array of `argc` C-string pointers, valid for the hook or server run.
///
/// # Safety
///
/// A non-null `argv` must contain at least `argc` readable pointers. Non-null entries must be
/// valid C strings.
unsafe fn hook_args(argc: c_int, argv: *mut *mut c_char) -> Vec<String> {
    if argv.is_null() || argc <= 0 {
        return Vec::new();
    }
    (0..argc as usize)
        .filter_map(|i| {
            let p = *argv.add(i);
            if p.is_null() {
                None
            } else {
                Some(CStr::from_ptr(p).to_string_lossy().into_owned())
            }
        })
        .collect()
}

/// `tpsvrthrinit` trampoline, run on each dispatch thread.
///
/// # Arguments
///
/// - `argc`: Number of entries in the native argument array.
/// - `argv`: Readable array of `argc` C-string pointers, valid for the hook or server run.
///
/// Enduro/X calls this from `ndrx_call_tpsvrthrinit`, which has already done
/// `tpinit(NULL)` for the worker, so the thread has a usable ATMI context. The
/// library default runs first for its `tx_open()`; a Rust hook runs after it and
/// on top of it, never instead.
///
/// # Safety
///
/// Enduro/X must have initialized the worker’s ATMI TLS. The argument array and registered
/// hooks must remain valid during the call.
unsafe extern "C" fn rust_thread_init(argc: c_int, argv: *mut *mut c_char) -> c_int {
    /// Keep the first failure only: later workers may fail for knock-on
    /// reasons, and the first one is the useful diagnosis.
    ///
    /// # Arguments
    ///
    /// - `err`: Worker initialization error to preserve if no earlier worker error was recorded.
    fn record(err: AtmiError) -> c_int {
        if let Ok(mut rt) = server_runtime().lock() {
            if rt.thread_init_error.is_none() {
                rt.thread_init_error = Some(err);
            }
        }
        raw::EXFAIL as c_int
    }

    if raw::tpsvrthrinit(argc, argv) < 0 {
        return record(AtmiError::new(
            raw::TPESYSTEM,
            "Enduro/X default tpsvrthrinit() failed for a dispatch thread",
        ));
    }

    let hook = match server_runtime().lock() {
        Ok(rt) => rt.thread_init_hook,
        Err(_) => return record(runtime_lock_err()),
    };
    let Some(hook) = hook else {
        return raw::EXSUCCEED as c_int;
    };

    let ctx = match AtmiCtx::borrow_current_context() {
        Ok(ctx) => ctx,
        Err(err) => return record(err),
    };
    let args = hook_args(argc, argv);

    match catch_unwind(AssertUnwindSafe(|| hook(&ctx, &args))) {
        Ok(Ok(())) => raw::EXSUCCEED as c_int,
        Ok(Err(err)) => record(err),
        Err(_) => record(AtmiError::new(
            raw::TPESYSTEM,
            "server thread init hook panicked",
        )),
    }
}

/// `tpsvrthrdone` trampoline, run on each dispatch thread.
///
/// `ndrx_call_tpsvrthrdone` invokes this *before* `tpterm()`, so the worker
/// context is still live. The Rust hook runs first, then the library default
/// for its `tx_close()`.
///
/// # Safety
///
/// Call on the initialized worker before its ATMI session is terminated, while the server
/// runner remains active.
unsafe extern "C" fn rust_thread_done() {
    let hook = match server_runtime().lock() {
        Ok(rt) => rt.thread_done_hook,
        Err(_) => None,
    };

    if let Some(hook) = hook {
        if let Ok(ctx) = AtmiCtx::borrow_current_context() {
            let _ = catch_unwind(AssertUnwindSafe(|| hook(&ctx)));
        }
    }

    raw::tpsvrthrdone();
}

/// Run the Rust main initialization hook and preserve its error for the server runner.
///
/// # Arguments
///
/// - `argc`: Number of entries in the native argument array.
/// - `argv`: Readable array of `argc` C-string pointers, valid for the hook or server run.
///
/// # Safety
///
/// The runner’s context and hook must remain alive; `argv` must contain `argc` readable
/// C-string pointers when non-null.
unsafe extern "C" fn rust_server_init(argc: c_int, argv: *mut *mut c_char) -> c_int {
    let (ctx_addr, init_hook) = match server_runtime().lock() {
        Ok(rt) => (rt.ctx_addr, rt.init_hook),
        Err(_) => return raw::EXFAIL as c_int,
    };

    if ctx_addr == 0 {
        return raw::EXFAIL as c_int;
    }

    let Some(init_cb) = init_hook else {
        return raw::EXFAIL as c_int;
    };

    let args: Vec<String> = if argv.is_null() || argc <= 0 {
        Vec::new()
    } else {
        (0..argc as usize)
            .filter_map(|i| {
                let p = *argv.add(i);
                if p.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(p).to_string_lossy().into_owned())
                }
            })
            .collect()
    };

    let ctx = &*(ctx_addr as *const AtmiCtx);
    match catch_unwind(AssertUnwindSafe(|| init_cb(ctx, &args))) {
        Ok(Ok(())) => raw::EXSUCCEED as c_int,
        Ok(Err(err)) => {
            if let Ok(mut rt) = server_runtime().lock() {
                rt.init_error = Some(err);
            }
            raw::EXFAIL as c_int
        }
        Err(_) => {
            if let Ok(mut rt) = server_runtime().lock() {
                rt.init_error = Some(AtmiError::new(
                    raw::TPESYSTEM,
                    "panic in server init callback",
                ));
            }
            raw::EXFAIL as c_int
        }
    }
}

/// Run the Rust main shutdown hook without allowing a panic across the C boundary.
///
/// # Safety
///
/// Call on the main server thread while the runner’s context and shutdown hook remain alive.
unsafe extern "C" fn rust_server_done() {
    let (ctx_addr, done_hook) = match server_runtime().lock() {
        Ok(rt) => (rt.ctx_addr, rt.done_hook),
        Err(_) => return,
    };

    if ctx_addr == 0 {
        return;
    }

    let Some(done_cb) = done_hook else {
        return;
    };

    let ctx = &*(ctx_addr as *const AtmiCtx);
    let _ = catch_unwind(AssertUnwindSafe(|| done_cb(ctx)));
}

/// Service dispatch, ownership transfer, and native server-loop integration.
impl AtmiCtx {
    /// Translate a zero native status to success and any other status to the current ATMI error.
    ///
    /// # Arguments
    ///
    /// - `rc`: Native return status; only `EXSUCCEED` is treated as success.
    ///
    #[inline]
    pub(crate) fn rc_to_result(&self, rc: c_int) -> AtmiResult<()> {
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Advertise a service using a native C callback and an explicit function name.
    ///
    /// # Arguments
    ///
    /// - `svc_nm`: Advertised service name, without embedded NUL bytes.
    /// - `p_func`: C-compatible service entry point; `None` passes a null callback to Enduro/X.
    /// - `fn_nm`: Function name recorded in service metadata, which may differ from the service
    ///   name.
    pub fn tpadvertise_full(
        &self,
        svc_nm: &str,
        p_func: Option<ServiceCallback>,
        fn_nm: &str,
    ) -> AtmiResult<()> {
        let c_svc = CString::new(svc_nm)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "svc_nm contains NUL"))?;
        let c_fn =
            CString::new(fn_nm).map_err(|_| AtmiError::new(raw::TPEINVAL, "fn_nm contains NUL"))?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpadvertise_full(c_svc.as_ptr(), p_func, c_fn.as_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpadvertise_full(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                p_func,
                c_fn.as_ptr() as *mut c_char,
            )
        };

        self.rc_to_result(rc)
    }

    /// High-level Rust service advertisement.
    ///
    /// # Arguments
    ///
    /// - `svc_nm`: Advertised service name, without embedded NUL bytes.
    /// - `handler`: Rust handler receiving the active context and mutable service request metadata.
    ///
    /// This avoids `extern "C"` callbacks in user code. Register these from
    /// the `tp_run(...)` init hook.
    pub fn tpadvertise(&self, svc_nm: &str, handler: RustServiceCallback) -> AtmiResult<()> {
        let self_addr = self as *const AtmiCtx as usize;
        {
            let mut rt = server_runtime().lock().map_err(|_| runtime_lock_err())?;
            if rt.ctx_addr != self_addr {
                return Err(AtmiError::new(
                    raw::TPEPROTO,
                    "tpadvertise() must be called from the active tp_run() context",
                ));
            }
            rt.services.insert(svc_nm.to_owned(), handler);
        }

        if let Err(err) = self.tpadvertise_full(svc_nm, Some(rust_service_dispatch), svc_nm) {
            if let Ok(mut rt) = server_runtime().lock() {
                rt.services.remove(svc_nm);
            }
            return Err(err);
        }
        Ok(())
    }

    /// Remove a service advertisement and its matching Rust handler registration.
    ///
    /// # Arguments
    ///
    /// - `svc_nm`: Advertised service name, without embedded NUL bytes.
    pub fn tpunadvertise(&self, svc_nm: &str) -> AtmiResult<()> {
        let c_svc = CString::new(svc_nm)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "svc_nm contains NUL"))?;
        let self_addr = self as *const AtmiCtx as usize;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpunadvertise(c_svc.as_ptr() as *mut c_char) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpunadvertise(self.c_ctx_ptr(), c_svc.as_ptr() as *mut c_char) };

        self.rc_to_result(rc)?;
        if let Ok(mut rt) = server_runtime().lock() {
            if rt.ctx_addr == self_addr {
                rt.services.remove(svc_nm);
            }
        }
        Ok(())
    }

    /// Hand a reply to Enduro/X through native `tpreturn` or `tpforward`.
    ///
    /// # Arguments
    ///
    /// - `reply`: Operation, buffer and flags; buffer ownership transfers to Enduro/X.
    ///
    /// # Safety
    ///
    /// The buffer must be valid and exclusively available for ownership transfer, and no Rust
    /// wrapper may still refer to the request buffer of the current call unless it is the
    /// buffer being passed. Call only during service dispatch with long jumps disabled.
    unsafe fn reply_value(&self, reply: ServiceReply) {
        match reply.kind {
            ReplyKind::Return { rval, rcode } => {
                #[cfg(not(feature = "ctx-send"))]
                raw::tpreturn(rval, rcode, reply.data, reply.len, reply.flags);

                #[cfg(feature = "ctx-send")]
                raw::Otpreturn(
                    self.c_ctx_ptr(),
                    rval,
                    rcode,
                    reply.data,
                    reply.len,
                    reply.flags,
                );
            }
            ReplyKind::Forward { svc } => {
                #[cfg(not(feature = "ctx-send"))]
                raw::tpforward(
                    svc.as_ptr() as *mut c_char,
                    reply.data,
                    reply.len,
                    reply.flags,
                );

                #[cfg(feature = "ctx-send")]
                raw::Otpforward(
                    self.c_ctx_ptr(),
                    svc.as_ptr() as *mut c_char,
                    reply.data,
                    reply.len,
                    reply.flags,
                );
            }
        }
    }

    /// Issue a handler's reply now, or park it until the handler has returned.
    ///
    /// # Arguments
    ///
    /// - `kind`: Native operation to perform.
    /// - `data`: Reply or forwarded buffer; consumed on every path.
    /// - `flags`: Native service-operation flags; use `0` for default behavior.
    ///
    /// See [`DispatchFrame`] for why a reply can be held back. Outside a Rust
    /// dispatch (a native callback registered through
    /// [`Self::tpadvertise_full`]) the reply always goes out at once.
    fn submit_reply(&self, kind: ReplyKind, data: TypedBuffer<'_>, flags: i64) {
        enum Action {
            Now,
            Park,
            Discard,
        }

        let data_is_request = data.is_request();
        let action = DISPATCH_FRAME.with(|slot| match slot.borrow_mut().as_mut() {
            None => Action::Now,
            Some(frame) if frame.replied => Action::Discard,
            Some(frame) => {
                frame.replied = true;
                if data_is_request || frame.request_released {
                    Action::Now
                } else {
                    Action::Park
                }
            }
        });

        if matches!(action, Action::Discard) {
            // A second reply for one call: the first stands, free this buffer.
            return;
        }

        let reply = ServiceReply {
            kind,
            len: data.len() as c_long,
            data: data.into_raw(),
            flags: flags as c_long,
        };
        match action {
            Action::Now => unsafe { self.reply_value(reply) },
            Action::Park => DISPATCH_FRAME.with(|slot| {
                if let Some(frame) = slot.borrow_mut().as_mut() {
                    frame.pending = Some(reply);
                }
            }),
            Action::Discard => unreachable!("handled above"),
        }
    }

    /// Forward a UBF request from the current service to another service.
    ///
    /// # Arguments
    ///
    /// - `svc`: Destination service name, without embedded NUL bytes.
    /// - `data`: Reply or forwarded buffer whose ownership transfers to Enduro/X.
    /// - `flags`: Native service-operation flags; use `0` for default behavior.
    ///
    /// This consumes `data` and transfers ownership to Enduro/X. The function
    /// returns normally under `tp_run`; return from the handler after forwarding.
    /// As with [`Self::tpreturn`], the forward is issued when the handler returns
    /// if the handler still holds the request buffer and forwards a different one.
    pub fn tpforward_ubf(&self, svc: &str, data: TypedUbf<'_>, flags: i64) {
        let Ok(svc) = CString::new(svc) else {
            self.tpreturn_ubf(TpReturnStatus::Fail, raw::TPEINVAL as i64, data, 0);
            return;
        };
        self.submit_reply(ReplyKind::Forward { svc }, data.into_inner(), flags);
    }

    /// Request termination of the native server from a service callback.
    ///
    /// # Safety
    ///
    /// Call only from a native service callback with control-flow settings that cannot
    /// long-jump across Rust frames.
    pub(crate) unsafe fn tpexit(&self) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpexit();

        #[cfg(feature = "ctx-send")]
        raw::Otpexit(self.c_ctx_ptr());
    }

    /// Release the current service dispatch without sending a reply.
    ///
    /// # Safety
    ///
    /// Call only during service dispatch with long jumps disabled, preserving any request
    /// ownership needed for later completion.
    pub(crate) unsafe fn tpcontinue(&self) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpcontinue();

        #[cfg(feature = "ctx-send")]
        raw::Otpcontinue(self.c_ctx_ptr());
    }

    /// Capture native service context data for transfer to another thread.
    ///
    /// # Safety
    ///
    /// Call during an active service invocation. The returned allocation must be released with
    /// `tpsrvfreectxdata`.
    pub(crate) unsafe fn tpsrvgetctxdata(&self) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::tpsrvgetctxdata()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Otpsrvgetctxdata(self.c_ctx_ptr())
        }
    }

    /// Capture service context data using caller-provided or native-allocated storage.
    ///
    /// # Arguments
    ///
    /// - `p_buf`: Writable context-data storage, or null to request a native allocation.
    /// - `p_len`: Writable length slot carrying storage capacity on input and the captured size
    ///   on output.
    ///
    /// # Safety
    ///
    /// Call during active service dispatch. `p_len` must be writable, and a non-null `p_buf`
    /// must satisfy the native storage and allocation contract.
    pub(crate) unsafe fn tpsrvgetctxdata2(
        &self,
        p_buf: *mut c_char,
        p_len: *mut c_long,
    ) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::tpsrvgetctxdata2(p_buf, p_len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Otpsrvgetctxdata2(self.c_ctx_ptr(), p_buf, p_len)
        }
    }

    /// Free context data returned by the native service-context capture API.
    ///
    /// # Arguments
    ///
    /// - `p_buf`: Owned native context-data allocation to release exactly once.
    ///
    /// # Safety
    ///
    /// The pointer must be a live native context-data allocation owned by the caller and must
    /// not be used after this call.
    pub(crate) unsafe fn tpsrvfreectxdata(&self, p_buf: *mut c_char) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpsrvfreectxdata(p_buf);

        #[cfg(feature = "ctx-send")]
        raw::Otpsrvfreectxdata(self.c_ctx_ptr(), p_buf);
    }

    /// Restore previously captured service context data on the current context.
    ///
    /// # Arguments
    ///
    /// - `data`: Valid context-data block captured by `tpsrvgetctxdata` or `tpsrvgetctxdata2`.
    /// - `flags`: Native service-operation flags; use `0` for default behavior.
    pub(crate) fn tpsrvsetctxdata(&self, data: *mut c_char, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsrvsetctxdata(data, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsrvsetctxdata(self.c_ctx_ptr(), data, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Register a descriptor and callback in the server’s main event loop.
    ///
    /// # Arguments
    ///
    /// - `fd`: File descriptor to register, remove, or dispatch; ownership stays with the caller.
    /// - `events`: Native poll-event bitmask selecting or reporting descriptor readiness.
    /// - `user_data`: Application value copied into each `PollerEvent`; no pointer ownership is
    ///   inferred.
    /// - `callback`: Function called with the main server context; return `0` for success or
    ///   `-1` for failure.
    ///
    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_addpollerfd(
        &self,
        fd: i32,
        events: u32,
        user_data: usize,
        callback: RustPollerCallback,
    ) -> AtmiResult<()> {
        require_main_thread("tpext_addpollerfd")?;
        {
            let mut rt = extension_runtime().lock().map_err(|_| runtime_lock_err())?;
            rt.pollers.insert(fd, (callback, user_data));
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpext_addpollerfd(
                fd as c_int,
                events,
                std::ptr::null_mut(),
                Some(rust_poller_dispatch),
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpext_addpollerfd(
                self.c_ctx_ptr(),
                fd as c_int,
                events,
                std::ptr::null_mut(),
                Some(rust_poller_dispatch),
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.pollers.remove(&fd);
            }
            Err(self.atmi_last_error())
        }
    }

    /// Remove a descriptor’s poll callback without closing the descriptor.
    ///
    /// # Arguments
    ///
    /// - `fd`: File descriptor to register, remove, or dispatch; ownership stays with the caller.
    ///
    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_delpollerfd(&self, fd: i32) -> AtmiResult<()> {
        require_main_thread("tpext_delpollerfd")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_delpollerfd(fd as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpext_delpollerfd(self.c_ctx_ptr(), fd as c_int) };

        if rc == raw::EXSUCCEED as c_int {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.pollers.remove(&fd);
            }
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Register the callback that runs at a configured interval on the main server thread.
    ///
    /// # Arguments
    ///
    /// - `secs`: Positive interval in seconds between periodic callback invocations.
    /// - `callback`: Function called with the main server context; return `0` for success or
    ///   `-1` for failure.
    ///
    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_addperiodcb(&self, secs: i32, callback: RustPeriodCallback) -> AtmiResult<()> {
        require_main_thread("tpext_addperiodcb")?;
        {
            let mut rt = extension_runtime().lock().map_err(|_| runtime_lock_err())?;
            rt.period_cb = Some(callback);
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_addperiodcb(secs as c_int, Some(rust_period_dispatch)) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpext_addperiodcb(self.c_ctx_ptr(), secs as c_int, Some(rust_period_dispatch))
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.period_cb = None;
            }
            Err(self.atmi_last_error())
        }
    }

    /// Remove the server’s periodic callback.
    ///
    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_delperiodcb(&self) -> AtmiResult<()> {
        require_main_thread("tpext_delperiodcb")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_delperiodcb() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpext_delperiodcb(self.c_ctx_ptr()) };

        if rc == raw::EXSUCCEED as c_int {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.period_cb = None;
            }
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Register the callback that runs before each main-loop poll.
    ///
    /// # Arguments
    ///
    /// - `callback`: Function called with the main server context; return `0` for success or
    ///   `-1` for failure.
    ///
    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_addb4pollcb(&self, callback: RustBeforePollCallback) -> AtmiResult<()> {
        require_main_thread("tpext_addb4pollcb")?;
        {
            let mut rt = extension_runtime().lock().map_err(|_| runtime_lock_err())?;
            rt.before_poll_cb = Some(callback);
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_addb4pollcb(Some(rust_before_poll_dispatch)) };

        #[cfg(feature = "ctx-send")]
        let rc =
            unsafe { raw::Otpext_addb4pollcb(self.c_ctx_ptr(), Some(rust_before_poll_dispatch)) };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.before_poll_cb = None;
            }
            Err(self.atmi_last_error())
        }
    }

    /// Remove the server’s before-poll callback.
    ///
    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_delb4pollcb(&self) -> AtmiResult<()> {
        require_main_thread("tpext_delb4pollcb")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_delb4pollcb() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpext_delb4pollcb(self.c_ctx_ptr()) };

        if rc == raw::EXSUCCEED as c_int {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.before_poll_cb = None;
            }
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Return the configured identifier of this server instance.
    pub fn tpgetsrvid(&self) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetsrvid() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetsrvid(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Run the native server entry point using C command-line arguments.
    ///
    /// # Arguments
    ///
    /// - `argc`: Number of entries in the native argument array.
    /// - `argv`: Readable array of `argc` C-string pointers, valid for the hook or server run.
    ///
    /// # Safety
    ///
    /// The arguments must remain valid throughout the server run. Native control flow must not
    /// long-jump across Rust callbacks.
    pub(crate) unsafe fn ndrx_main(&self, argc: i32, argv: *mut *mut c_char) -> i32 {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::ndrx_main(argc as c_int, argv)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Ondrx_main(self.c_ctx_ptr(), argc as c_int, argv)
        }
    }

    /// Run the native server entry point with explicit lifecycle callbacks and integration flags.
    ///
    /// # Arguments
    ///
    /// - `argc`: Number of entries in the native argument array.
    /// - `argv`: Readable array of `argc` C-string pointers, valid for the hook or server run.
    /// - `in_tpsvrinit`: Optional native initialization callback, valid for the server run.
    /// - `in_tpsvrdone`: Optional native shutdown callback, valid for the server run.
    /// - `flags`: Server integration options; Rust dispatch requires `ATMI_SRVLIB_NOLONGJUMP`.
    ///
    /// # Safety
    ///
    /// Argument strings and callback pointers must remain valid throughout the server run. Rust
    /// callbacks require `ATMI_SRVLIB_NOLONGJUMP`.
    pub(crate) unsafe fn ndrx_main_integra(
        &self,
        argc: i32,
        argv: *mut *mut c_char,
        in_tpsvrinit: Option<ServerInitHook>,
        in_tpsvrdone: Option<ServerDoneHook>,
        flags: i64,
    ) -> i32 {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::ndrx_main_integra(
                argc as c_int,
                argv,
                in_tpsvrinit,
                in_tpsvrdone,
                flags as c_long,
            )
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Ondrx_main_integra(
                self.c_ctx_ptr(),
                argc as c_int,
                argv,
                in_tpsvrinit,
                in_tpsvrdone,
                flags as c_long,
            )
        }
    }

    /// High-level server runner: hands the four C lifecycle hooks to Enduro/X
    /// and does not return until the server shuts down.
    ///
    /// # Arguments
    ///
    /// - `hooks`: Main and optional worker lifecycle hooks; services are advertised from the
    ///   main init hook.
    ///
    /// Handles argv conversion and the low-level C callback wiring.
    ///
    /// ```no_run
    ///
    /// # use endurox_rs::{AtmiCtx, AtmiResult, ServerHooks};
    /// # fn my_init(_: &AtmiCtx, _: &[String]) -> AtmiResult<()> { Ok(()) }
    /// # fn my_done(_: &AtmiCtx) {}
    /// # fn my_thread_init(_: &AtmiCtx, _: &[String]) -> AtmiResult<()> { Ok(()) }
    /// # fn run(ctx: &AtmiCtx) -> AtmiResult<()> {
    /// ctx.tp_run(
    ///     ServerHooks::new(my_init)
    ///         .done(my_done)
    ///         .thread_init(my_thread_init),
    /// )
    /// # }
    /// ```
    pub fn tp_run(&self, hooks: ServerHooks) -> AtmiResult<()> {
        self.tp_run_inner(hooks)
    }

    /// Install server state, run Enduro/X, and report the saved initialization failure if
    /// startup fails.
    ///
    /// # Arguments
    ///
    /// - `hooks`: Main and optional worker lifecycle hooks; services are advertised from the
    ///   main init hook.
    fn tp_run_inner(&self, hooks: ServerHooks) -> AtmiResult<()> {
        let self_addr = self as *const AtmiCtx as usize;
        {
            let mut rt = server_runtime().lock().map_err(|_| runtime_lock_err())?;
            if rt.ctx_addr != 0 {
                return Err(AtmiError::new(
                    raw::TPEPROTO,
                    "a server runtime is already active in this process",
                ));
            }
            rt.reset();
            rt.ctx_addr = self_addr;
            rt.main_thread_id = Some(std::thread::current().id());
            rt.init_hook = Some(hooks.init);
            rt.done_hook = hooks.done;
            rt.thread_init_hook = hooks.thread_init;
            rt.thread_done_hook = hooks.thread_done;
        }

        let _runtime_guard = ServerRuntimeGuard;
        let _thread_mode_guard = unsafe { ServerThreadModeGuard::enable() };

        use std::os::unix::ffi::OsStrExt;
        let mut cargs: Vec<CString> = std::env::args_os()
            .map(|s| {
                CString::new(s.as_bytes())
                    .map_err(|_| AtmiError::new(raw::TPEINVAL, "argv contains NUL byte"))
            })
            .collect::<Result<_, _>>()?;
        let mut argv: Vec<*mut c_char> = cargs
            .iter_mut()
            .map(|s| s.as_ptr() as *mut c_char)
            .collect();

        let rc = unsafe {
            self.ndrx_main_integra(
                argv.len() as c_int,
                argv.as_mut_ptr(),
                Some(rust_server_init),
                Some(rust_server_done),
                raw::ATMI_SRVLIB_NOLONGJUMP as i64,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            let init_error = server_runtime().lock().ok().and_then(|rt| {
                // Prefer the main init hook's error; fall back to the first
                // worker-thread failure, which is otherwise invisible.
                rt.init_error
                    .clone()
                    .or_else(|| rt.thread_init_error.clone())
            });
            Err(init_error.unwrap_or_else(|| AtmiError::new(raw::TPESYSTEM, "ATMI server failed")))
        }
    }

    /// Return a typed buffer from a service callback.
    ///
    /// # Arguments
    ///
    /// - `status`: Whether the service completed successfully or failed.
    /// - `rcode`: Application-defined service return code delivered to the caller.
    /// - `data`: Reply or forwarded buffer whose ownership transfers to Enduro/X.
    /// - `flags`: Native service-operation flags; use `0` for default behavior.
    ///
    /// Consumes `data` so its `Drop` is **not** called — ownership is
    /// transferred to the XATMI framework. The buffer's tracked `len()` is
    /// forwarded as the `tpreturn` length argument (relevant for CARRAY/STRING;
    /// ignored for self-describing buffer types).
    ///
    /// The reply is sent immediately when `data` is the request buffer or the
    /// request buffer has already been dropped. If the handler still holds the
    /// request and replies with a different buffer, the reply is sent when the
    /// handler returns: Enduro/X frees the request as part of replying, which
    /// must not happen while Rust code can still use it. Only the first reply
    /// of a call is sent; a later one is discarded and its buffer freed.
    pub fn tpreturn(&self, status: TpReturnStatus, rcode: i64, data: TypedBuffer<'_>, flags: i64) {
        self.submit_reply(
            ReplyKind::Return {
                rval: status.to_raw(),
                rcode: rcode as c_long,
            },
            data,
            flags,
        );
    }

    /// Return a UBF buffer from a service callback.
    ///
    /// # Arguments
    ///
    /// - `status`: Whether the service completed successfully or failed.
    /// - `rcode`: Application-defined service return code delivered to the caller.
    /// - `data`: Reply or forwarded buffer whose ownership transfers to Enduro/X.
    /// - `flags`: Native service-operation flags; use `0` for default behavior.
    ///
    /// Convenience wrapper over [`AtmiCtx::tpreturn`] for the common UBF case.
    pub fn tpreturn_ubf(&self, status: TpReturnStatus, rcode: i64, data: TypedUbf<'_>, flags: i64) {
        self.tpreturn(status, rcode, data.into_inner(), flags);
    }
}
