use crate::{raw, AtmiCtx, AtmiError, AtmiResult, TpSvcInfo, TypedBuffer, TypedUbf};
use core::ffi::{c_char, c_int, c_long};
use std::ffi::CString;
use std::{
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
/// Returning `Err(...)` aborts server startup.
pub type RustServerInitHook = fn(&AtmiCtx) -> AtmiResult<()>;

/// High-level server shutdown callback.
pub type RustServerDoneHook = fn(&AtmiCtx);

/// High-level service callback used by [`AtmiCtx::tpadvertise`].
pub type RustServiceCallback = for<'ctx> fn(&'ctx AtmiCtx, &mut TpSvcInfo<'ctx>);

/// Event delivered to a Rust poller callback registered with
/// [`AtmiCtx::tpext_addpollerfd`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollerEvent {
    pub fd: i32,
    pub events: u32,
    pub user_data: usize,
}

/// High-level poller callback used by [`AtmiCtx::tpext_addpollerfd`].
pub type RustPollerCallback = fn(PollerEvent) -> i32;

/// High-level periodic callback used by [`AtmiCtx::tpext_addperiodcb`].
pub type RustPeriodCallback = fn() -> i32;

/// High-level before-poll callback used by [`AtmiCtx::tpext_addb4pollcb`].
pub type RustBeforePollCallback = fn() -> i32;

/// Service return status for [`AtmiCtx::tpreturn_buffer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpReturnStatus {
    Success,
    Fail,
}

impl TpReturnStatus {
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
    init_hook: Option<RustServerInitHook>,
    done_hook: Option<RustServerDoneHook>,
    init_error: Option<AtmiError>,
    services: HashMap<String, RustServiceCallback>,
}

impl ServerRuntime {
    fn reset(&mut self) {
        self.ctx_addr = 0;
        self.init_hook = None;
        self.done_hook = None;
        self.init_error = None;
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

#[inline]
fn server_runtime() -> &'static Mutex<ServerRuntime> {
    SERVER_RUNTIME.get_or_init(|| Mutex::new(ServerRuntime::default()))
}

#[inline]
fn extension_runtime() -> &'static Mutex<ExtensionRuntime> {
    EXTENSION_RUNTIME.get_or_init(|| Mutex::new(ExtensionRuntime::default()))
}

#[inline]
fn runtime_lock_err() -> AtmiError {
    AtmiError::new(raw::TPESYSTEM, "server runtime state is poisoned")
}

struct ServerRuntimeGuard;

impl Drop for ServerRuntimeGuard {
    fn drop(&mut self) {
        if let Ok(mut rt) = server_runtime().lock() {
            rt.reset();
        }
    }
}

unsafe extern "C" fn rust_service_dispatch(svc_ptr: *mut raw::TPSVCINFO) {
    if svc_ptr.is_null() {
        return;
    }

    let ctx_addr = match server_runtime().lock() {
        Ok(rt) => rt.ctx_addr,
        Err(_) => return,
    };

    if ctx_addr == 0 {
        return;
    }

    let ctx = &*(ctx_addr as *const AtmiCtx);
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

    match cb {
        Some(handler) => {
            if catch_unwind(AssertUnwindSafe(|| handler(ctx, &mut svc))).is_err() {
                // Handler panicked. If it hadn't consumed the data buffer yet,
                // use it for the error response; otherwise allocate a fresh one.
                let err_ptr = match svc.take_data() {
                    Some(buf) => buf.into_raw(),
                    None => ctx
                        .tpalloc_ubf(256)
                        .map(|u| u.into_inner().into_raw())
                        .unwrap_or(std::ptr::null_mut()),
                };
                ctx.tpreturn(TpReturnStatus::Fail.to_raw(), 0, err_ptr, 0, 0);
            }
        }
        None => {
            let err_ptr = svc
                .take_data()
                .map(|b| b.into_raw())
                .unwrap_or(std::ptr::null_mut());
            ctx.tpreturn(TpReturnStatus::Fail.to_raw(), 0, err_ptr, 0, 0);
        }
    }
}

unsafe extern "C" fn rust_poller_dispatch(
    fd: c_int,
    events: u32,
    _ptr1: *mut ::std::os::raw::c_void,
) -> c_int {
    let (cb, user_data) = match extension_runtime().lock() {
        Ok(rt) => match rt.pollers.get(&(fd as i32)).copied() {
            Some(v) => v,
            None => return 0,
        },
        Err(_) => return -1,
    };

    catch_unwind(AssertUnwindSafe(|| {
        cb(PollerEvent {
            fd: fd as i32,
            events,
            user_data,
        })
    }))
    .unwrap_or(-1)
}

unsafe extern "C" fn rust_period_dispatch() -> c_int {
    let cb = match extension_runtime().lock() {
        Ok(rt) => rt.period_cb,
        Err(_) => return -1,
    };

    match cb {
        Some(cb) => catch_unwind(AssertUnwindSafe(cb)).unwrap_or(-1),
        None => 0,
    }
}

unsafe extern "C" fn rust_before_poll_dispatch() -> c_int {
    let cb = match extension_runtime().lock() {
        Ok(rt) => rt.before_poll_cb,
        Err(_) => return -1,
    };

    match cb {
        Some(cb) => catch_unwind(AssertUnwindSafe(cb)).unwrap_or(-1),
        None => 0,
    }
}

unsafe extern "C" fn rust_server_init(_argc: c_int, _argv: *mut *mut c_char) -> c_int {
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

    let ctx = &*(ctx_addr as *const AtmiCtx);
    match catch_unwind(AssertUnwindSafe(|| init_cb(ctx))) {
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

impl AtmiCtx {
    #[inline]
    pub(crate) fn rc_to_result(&self, rc: c_int) -> AtmiResult<()> {
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpadvertise_full(
        &self,
        svc_nm: &str,
        p_func: Option<ServiceCallback>,
        fn_nm: &str,
    ) -> AtmiResult<()> {
        let c_svc = CString::new(svc_nm).map_err(|_| self.atmi_last_error())?;
        let c_fn = CString::new(fn_nm).map_err(|_| self.atmi_last_error())?;

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

    pub fn tpunadvertise(&self, svc_nm: &str) -> AtmiResult<()> {
        let c_svc = CString::new(svc_nm).map_err(|_| self.atmi_last_error())?;
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

    pub(crate) unsafe fn tpreturn(
        &self,
        rval: i32,
        rcode: i64,
        data: *mut c_char,
        len: usize,
        flags: i64,
    ) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpreturn(
            rval as c_int,
            rcode as c_long,
            data,
            len as c_long,
            flags as c_long,
        );

        #[cfg(feature = "ctx-send")]
        raw::Otpreturn(
            self.c_ctx_ptr(),
            rval as c_int,
            rcode as c_long,
            data,
            len as c_long,
            flags as c_long,
        );
    }

    pub(crate) unsafe fn tpforward(&self, svc: &str, data: *mut c_char, len: usize, flags: i64) {
        let c_svc = match CString::new(svc) {
            Ok(s) => s,
            Err(_) => return,
        };

        #[cfg(not(feature = "ctx-send"))]
        raw::tpforward(
            c_svc.as_ptr() as *mut c_char,
            data,
            len as c_long,
            flags as c_long,
        );

        #[cfg(feature = "ctx-send")]
        raw::Otpforward(
            self.c_ctx_ptr(),
            c_svc.as_ptr() as *mut c_char,
            data,
            len as c_long,
            flags as c_long,
        );
    }

    /// Forward a UBF request from the current service to another service.
    ///
    /// This consumes `data` and transfers ownership to Enduro/X. The function
    /// does not return to the caller in normal Enduro/X control flow.
    pub fn tpforward_ubf(&self, svc: &str, data: TypedUbf<'_>, flags: i64) {
        let ptr = data.into_raw();
        unsafe { self.tpforward(svc, ptr, 0, flags) };
    }

    pub(crate) unsafe fn tpexit(&self) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpexit();

        #[cfg(feature = "ctx-send")]
        raw::Otpexit(self.c_ctx_ptr());
    }

    pub(crate) unsafe fn tpcontinue(&self) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpcontinue();

        #[cfg(feature = "ctx-send")]
        raw::Otpcontinue(self.c_ctx_ptr());
    }

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

    pub(crate) unsafe fn tpsrvfreectxdata(&self, p_buf: *mut c_char) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpsrvfreectxdata(p_buf);

        #[cfg(feature = "ctx-send")]
        raw::Otpsrvfreectxdata(self.c_ctx_ptr(), p_buf);
    }

    pub(crate) fn tpsrvsetctxdata(&self, data: *mut c_char, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsrvsetctxdata(data, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsrvsetctxdata(self.c_ctx_ptr(), data, flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpext_addpollerfd(
        &self,
        fd: i32,
        events: u32,
        user_data: usize,
        callback: RustPollerCallback,
    ) -> AtmiResult<()> {
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

    pub fn tpext_delpollerfd(&self, fd: i32) -> AtmiResult<()> {
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

    pub fn tpext_addperiodcb(&self, secs: i32, callback: RustPeriodCallback) -> AtmiResult<()> {
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

    pub fn tpext_delperiodcb(&self) -> AtmiResult<()> {
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

    pub fn tpext_addb4pollcb(&self, callback: RustBeforePollCallback) -> AtmiResult<()> {
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

    pub fn tpext_delb4pollcb(&self) -> AtmiResult<()> {
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

    /// High-level server runner, analogous to Go `TpRun(init, uninit)`.
    ///
    /// This handles argv conversion and low-level C callback wiring.
    pub fn tp_run(
        &self,
        init_hook: RustServerInitHook,
        done_hook: RustServerDoneHook,
    ) -> AtmiResult<()> {
        self.tp_run_inner(init_hook, Some(done_hook))
    }

    /// Variant of [`AtmiCtx::tp_run`] without a shutdown callback.
    pub fn tp_run_no_uninit(&self, init_hook: RustServerInitHook) -> AtmiResult<()> {
        self.tp_run_inner(init_hook, None)
    }

    fn tp_run_inner(
        &self,
        init_hook: RustServerInitHook,
        done_hook: Option<RustServerDoneHook>,
    ) -> AtmiResult<()> {
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
            rt.init_hook = Some(init_hook);
            rt.done_hook = done_hook;
        }

        let _runtime_guard = ServerRuntimeGuard;

        let args: Vec<String> = std::env::args().collect();
        let mut cargs: Vec<CString> = args
            .iter()
            .map(|s| {
                CString::new(s.as_str())
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
                0,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            let init_error = server_runtime()
                .lock()
                .ok()
                .and_then(|rt| rt.init_error.clone());
            Err(init_error.unwrap_or_else(|| AtmiError::new(raw::TPESYSTEM, "ATMI server failed")))
        }
    }

    /// Return a typed buffer from a service callback.
    ///
    /// Consumes `data` so its `Drop` is **not** called — ownership is
    /// transferred to the XATMI framework via `tpreturn`.
    pub fn tpreturn_buffer(
        &self,
        status: TpReturnStatus,
        rcode: i64,
        data: TypedBuffer<'_>,
        flags: i64,
    ) {
        let ptr = data.into_raw();
        unsafe { self.tpreturn(status.to_raw(), rcode, ptr, 0, flags) };
    }

    /// Return a UBF buffer from a service callback.
    ///
    /// Convenience wrapper over [`AtmiCtx::tpreturn_buffer`] for the common UBF case.
    pub fn tpreturn_ubf(&self, status: TpReturnStatus, rcode: i64, data: TypedUbf<'_>, flags: i64) {
        self.tpreturn_buffer(status, rcode, data.into_inner(), flags);
    }
}
