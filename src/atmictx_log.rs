//! Application, middleware, and UBF logging with source locations and formatting macros.
use crate::raw;
use crate::{AtmiCtx, AtmiError, AtmiResult, TypedBuffer};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_long};

/// Native Enduro/X logging severity, from unconditional output to detailed dumps.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Always = 1,
    Error = 2,
    Warn = 3,
    Info = 4,
    Debug = 5,
    Dump = 6,
}

/// Conversion that preserves the source value’s native meaning.
impl From<LogLevel> for c_int {
    /// Convert a Rust log level to its native numeric value.
    ///
    /// # Arguments
    ///
    /// - `l`: Log severity to convert.
    fn from(l: LogLevel) -> Self {
        l as c_int
    }
}

/// Send a log message through a native logger, removing NUL bytes from the message.
///
/// # Arguments
///
/// - `f`: Native logging function to invoke synchronously.
/// - `level`: Message severity used by the configured log filter.
/// - `file`: Source file name, normally supplied by `file!()`; a NUL causes the message to be
///   skipped.
/// - `line`: Source line number, normally supplied by `line!()`.
/// - `msg`: Message text; embedded NUL bytes are removed before logging.
fn call_logex(
    f: impl FnOnce(c_int, *const c_char, c_long, *const c_char),
    level: LogLevel,
    file: &'static str,
    line: u32,
    msg: &str,
) {
    // sanitize and convert
    let c_file = match CString::new(file) {
        Ok(s) => s,
        Err(_) => return,
    };
    let c_msg = match CString::new(msg.replace('\0', "")) {
        Ok(s) => s,
        Err(_) => return,
    };
    f(
        level.into(),
        c_file.as_ptr(),
        line as c_long,
        c_msg.as_ptr(),
    )
}

/// Logging entry points that attach source-file and line information.
impl AtmiCtx {
    /// Write a message to the application logger, with its source location.
    ///
    /// # Arguments
    ///
    /// - `level`: Message severity used by the configured log filter.
    /// - `file`: Source file name, normally supplied by `file!()`; a NUL causes the message to
    ///   be skipped.
    /// - `line`: Source line number, normally supplied by `line!()`.
    /// - `msg`: Message text; embedded NUL bytes are removed before logging.
    ///
    #[inline]
    pub fn tplog_str(&self, level: LogLevel, file: &'static str, line: u32, msg: &str) {
        call_logex(
            |lev, file, line, msg| unsafe {
                #[cfg(not(feature = "ctx-send"))]
                raw::tplogex(lev, file, line, msg);
                #[cfg(feature = "ctx-send")]
                raw::Otplogex(self.c_ctx_ptr(), lev, file, line, msg);
            },
            level,
            file,
            line,
            msg,
        );
    }

    /// Write a message to the Enduro/X middleware logger, with its source location.
    ///
    /// # Arguments
    ///
    /// - `level`: Message severity used by the configured log filter.
    /// - `file`: Source file name, normally supplied by `file!()`; a NUL causes the message to
    ///   be skipped.
    /// - `line`: Source line number, normally supplied by `line!()`.
    /// - `msg`: Message text; embedded NUL bytes are removed before logging.
    ///
    #[inline]
    pub fn ndrxlog_str(&self, level: LogLevel, file: &'static str, line: u32, msg: &str) {
        call_logex(
            |lev, file, line, msg| unsafe {
                #[cfg(not(feature = "ctx-send"))]
                raw::ndrxlogex(lev, file, line, msg);
                #[cfg(feature = "ctx-send")]
                raw::Ondrxlogex(self.c_ctx_ptr(), lev, file, line, msg);
            },
            level,
            file,
            line,
            msg,
        );
    }

    /// Write a message to the UBF logger, with its source location.
    ///
    /// # Arguments
    ///
    /// - `level`: Message severity used by the configured log filter.
    /// - `file`: Source file name, normally supplied by `file!()`; a NUL causes the message to
    ///   be skipped.
    /// - `line`: Source line number, normally supplied by `line!()`.
    /// - `msg`: Message text; embedded NUL bytes are removed before logging.
    ///
    #[inline]
    pub fn ubflog_str(&self, level: LogLevel, file: &'static str, line: u32, msg: &str) {
        call_logex(
            |lev, file, line, msg| unsafe {
                #[cfg(not(feature = "ctx-send"))]
                raw::ubflogex(lev, file, line, msg);
                #[cfg(feature = "ctx-send")]
                raw::Oubflogex(self.c_ctx_ptr(), lev, file, line, msg);
            },
            level,
            file,
            line,
            msg,
        );
    }
}

/// Request logging shared by the application, middleware and UBF loggers.
impl AtmiCtx {
    /// Select a request log from a UBF, an explicit filename, or a filename service.
    ///
    /// `data` is optional. When supplied, a UBF stores the selected filename;
    /// `filesvc` may replace that buffer even on failure. The service must return
    /// UBF, since the native API does not return a payload length. Such replies
    /// retain a tracked length of zero. Without a UBF, supply `filename`.
    /// Filename strings must fit the native path limit and contain no NULs.
    /// Native field writes may fail for insufficient UBF space; grow it beforehand.
    /// Use a request filename distinct from the process log in threaded programs.
    pub fn tplogsetreqfile(
        &self,
        data: Option<&mut TypedBuffer<'_>>,
        filename: Option<&str>,
        filesvc: Option<&str>,
    ) -> AtmiResult<()> {
        let filename = filename.map(request_log_filename).transpose()?;
        let filesvc = filesvc
            .map(CString::new)
            .transpose()
            .map_err(|_| AtmiError::new(AtmiError::TPEINVAL, "filesvc contains NUL"))?;
        // Determine whether the native filename service can replace the payload.
        let is_ubf = data
            .as_ref()
            .map(|b| b.tptypes())
            .transpose()?
            .is_some_and(|info| matches!(info.type_name.as_str(), "UBF" | "FML" | "FML32"));
        if !is_ubf && filename.as_ref().is_none_or(|s| s.as_bytes().is_empty()) {
            return Err(AtmiError::new(
                AtmiError::TPEINVAL,
                "filename is required without a UBF",
            ));
        }
        let mut pointer = data.as_ref().map_or(std::ptr::null_mut(), |b| b.as_ptr());
        let slot = if data.is_some() {
            &mut pointer
        } else {
            std::ptr::null_mut()
        };
        let name = filename
            .as_ref()
            .map_or(std::ptr::null_mut(), |s| s.as_ptr() as *mut c_char);
        let service = filesvc
            .as_ref()
            .map_or(std::ptr::null_mut(), |s| s.as_ptr() as *mut c_char);
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tplogsetreqfile(slot, name, service) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otplogsetreqfile(self.c_ctx_ptr(), slot, name, service) };
        // Snapshot the diagnostic before any further native call.
        let result = self.rc_to_result(rc);
        if let Some(data) = data {
            data.replace_ptr(pointer);
            if is_ubf {
                data.set_len_reported(0);
            }
        }
        result
    }

    /// Read the request filename stored in a UBF. Missing fields return `TPENOENT`.
    pub fn tploggetbufreqfile(&self, data: &TypedBuffer<'_>) -> AtmiResult<String> {
        let mut filename = vec![0 as c_char; libc::PATH_MAX as usize];
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tploggetbufreqfile(
                data.as_ptr(),
                filename.as_mut_ptr(),
                filename.len() as c_int,
            )
        };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otploggetbufreqfile(
                self.c_ctx_ptr(),
                data.as_ptr(),
                filename.as_mut_ptr(),
                filename.len() as c_int,
            )
        };
        self.rc_to_result(rc)?;
        Ok(unsafe { CStr::from_ptr(filename.as_ptr()) }
            .to_string_lossy()
            .into_owned())
    }

    /// Delete the request filename field from a UBF without closing the current logger.
    pub fn tplogdelbufreqfile(&self, data: &mut TypedBuffer<'_>) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tplogdelbufreqfile(data.as_ptr()) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otplogdelbufreqfile(self.c_ctx_ptr(), data.as_ptr()) };
        self.rc_to_result(rc)
    }

    /// Return the active request filename, or `None` when request logging is closed.
    pub fn tploggetreqfile(&self) -> Option<String> {
        let mut filename = vec![0 as c_char; libc::PATH_MAX as usize];
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            // tploggetreqfile itself assumes NSTD TLS has already been initialized.
            raw::_Nget_Nerror_addr();
            raw::tploggetreqfile(filename.as_mut_ptr(), filename.len() as c_int)
        };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otploggetreqfile(
                self.c_ctx_ptr(),
                filename.as_mut_ptr(),
                filename.len() as c_int,
            )
        };
        (rc == 1).then(|| {
            unsafe { CStr::from_ptr(filename.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        })
    }

    /// Select a request filename directly, without modifying a buffer or calling a service.
    /// Native log-open failures use the native fallback logger and are not reported as errors.
    pub fn tplogsetreqfile_direct(&self, filename: &str) -> AtmiResult<()> {
        let filename = request_log_filename(filename)?;
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::tplogsetreqfile_direct(filename.as_ptr());
        }
        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::Otplogsetreqfile_direct(self.c_ctx_ptr(), filename.as_ptr());
        }
        Ok(())
    }

    /// Close all request loggers for this context.
    pub fn tplogclosereqfile(&self) {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::tplogclosereqfile();
        }
        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::Otplogclosereqfile(self.c_ctx_ptr());
        }
    }

    /// Close the context's thread loggers. Request loggers have separate lifetimes.
    pub fn tplogclosethread(&self) {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::tplogclosethread();
        }
        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::Otplogclosethread(self.c_ctx_ptr());
        }
    }
}

fn request_log_filename(filename: &str) -> AtmiResult<CString> {
    if filename.len() >= libc::PATH_MAX as usize {
        return Err(AtmiError::new(
            AtmiError::TPEINVAL,
            "request filename exceeds native path limit",
        ));
    }
    CString::new(filename)
        .map_err(|_| AtmiError::new(AtmiError::TPEINVAL, "request filename contains NUL"))
}

// ---- TP logger macros ----
/// Format and write a message to the application logger.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - `lvl`: Message severity as a `LogLevel`.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! tp_log {
    ($ctx:expr, $lvl:expr, $($arg:tt)*) => {{
        let __msg = format!($($arg)*);
        $ctx.tplog_str($lvl, file!(), line!(), &__msg);
    }};
}
/// Format and write a message to the application logger at `Always` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! tp_always { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Always, $($arg)*); } }
/// Format and write a message to the application logger at `Error` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! tp_error  { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Error,  $($arg)*); } }
/// Format and write a message to the application logger at `Warn` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! tp_warn   { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Warn,   $($arg)*); } }
/// Format and write a message to the application logger at `Info` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! tp_info   { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Info,   $($arg)*); } }
/// Format and write a message to the application logger at `Debug` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! tp_debug  { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Debug,  $($arg)*); } }
/// Format and write a message to the application logger at `Dump` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! tp_dump   { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Dump,   $($arg)*); } }

// ---- NDRX logger macros ----
/// Format and write a message to the middleware logger.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - `lvl`: Message severity as a `LogLevel`.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ndrx_log {
    ($ctx:expr, $lvl:expr, $($arg:tt)*) => {{
        let __msg = format!($($arg)*);
        $ctx.ndrxlog_str($lvl, file!(), line!(), &__msg);
    }};
}
/// Format and write a message to the middleware logger at `Always` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ndrx_always { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Always, $($arg)*); } }
/// Format and write a message to the middleware logger at `Error` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ndrx_error  { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Error,  $($arg)*); } }
/// Format and write a message to the middleware logger at `Warn` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ndrx_warn   { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Warn,   $($arg)*); } }
/// Format and write a message to the middleware logger at `Info` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ndrx_info   { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Info,   $($arg)*); } }
/// Format and write a message to the middleware logger at `Debug` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ndrx_debug  { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Debug,  $($arg)*); } }
/// Format and write a message to the middleware logger at `Dump` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ndrx_dump   { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Dump,   $($arg)*); } }

// ---- UBF logger macros ----
/// Format and write a message to the UBF logger.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - `lvl`: Message severity as a `LogLevel`.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ubf_log {
    ($ctx:expr, $lvl:expr, $($arg:tt)*) => {{
        let __msg = format!($($arg)*);
        $ctx.ubflog_str($lvl, file!(), line!(), &__msg);
    }};
}
/// Format and write a message to the UBF logger at `Always` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ubf_always { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Always, $($arg)*); } }
/// Format and write a message to the UBF logger at `Error` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ubf_error  { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Error,  $($arg)*); } }
/// Format and write a message to the UBF logger at `Warn` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ubf_warn   { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Warn,   $($arg)*); } }
/// Format and write a message to the UBF logger at `Info` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ubf_info   { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Info,   $($arg)*); } }
/// Format and write a message to the UBF logger at `Debug` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ubf_debug  { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Debug,  $($arg)*); } }
/// Format and write a message to the UBF logger at `Dump` level.
///
/// # Arguments
///
/// - `ctx`: ATMI context used for logging.
/// - Remaining arguments: A format string and its values, as accepted by `format!`.
#[macro_export]
macro_rules! ubf_dump   { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Dump,   $($arg)*); } }
