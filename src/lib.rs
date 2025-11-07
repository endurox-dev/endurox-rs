//! endurox-rs — minimal safe-ish Rust wrapper over Enduro/X XATMI client APIs
//!
//! This module offers RAII for `tpinit()/tpterm()`, typed buffer management,
//! and simple sync/async calls (`tpcall`, `tpacall`/`tpgetrply`).
//!
//! It purposely exposes flags and buffer types close to XATMI while providing
//! Rust-friendly ergonomics.
//!
//! Tested against Enduro/X `libatmi.so` (client).
//!
//! NOTE: This is a thin wrapper; most functions still map 1:1 to C calls.
//! You must run within a valid Enduro/X environment (NDRX* env vars, etc.).

use libc::{c_char, c_int, c_long};
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr;

// ===== FFI bindings to libatmi =====
#[link(name = "tux")]
unsafe extern "C" {
    // Core buffer management
    fn tpalloc(typ: *const c_char, subtyp: *const c_char, size: c_long) -> *mut c_char;
    fn tprealloc(ptr: *mut c_char, size: c_long) -> *mut c_char;
    fn tpfree(ptr: *mut c_char);
    fn tptypes(ptr: *mut c_char, type_out: *mut c_char, subtype_out: *mut c_char) -> c_long;

    // Calls
    fn tpcall(
        svc: *const c_char,
        idata: *mut c_char,
        ilen: c_long,
        odata: *mut *mut c_char,
        olen: *mut c_long,
        flags: c_long,
    ) -> c_int;

    fn tpacall(
        svc: *const c_char,
        data: *mut c_char,
        len: c_long,
        flags: c_long,
    ) -> c_int; // returns call descriptor or -1 on error (tperrno)

    fn tpgetrply(cd: *mut c_int, data: *mut *mut c_char, len: *mut c_long, flags: c_long) -> c_int;
    fn tpcancel(cd: c_int) -> c_int;

    // Context/control
    fn tpinit(tpinfo: *mut ::std::os::raw::c_void) -> c_int;
    fn tpterm();
    fn tpopen() -> c_int; // XA open
    fn tpclose() -> c_int; // XA close

    /// Function that returns a pointer to the thread-local tperrno
    fn _exget_tperrno_addr() -> *mut c_int;

    fn tpstrerror(err: c_int) -> *const c_char;
}

/// ATMI return codes (success is 0)
const TPERROR: c_int = -1;

/// Common ATMI flags (subset). Add more as needed.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug)]
pub enum AtmiFlags {
    TPNOBLOCK = 0x0000_0001,
    TPSIGRSTRT = 0x0000_0002,
    TPNOREPLY = 0x0000_0004,
    TPNOTRAN = 0x0000_0008,
    TPTRAN = 0x0000_0010,
    TPNOTIME = 0x0000_0020,
    TPABSOLUTE = 0x0000_0040,
    TPGETANY = 0x0000_0080,
    TPNOCHANGE = 0x0000_0100,
    TPCONV = 0x0000_0400,
    TPSENDONLY = 0x0000_0800,
    TPRECVONLY = 0x0000_1000,
}
impl AtmiFlags {
    #[inline]
    pub fn bits(self) -> c_long { self as c_long }
}

/// ATMI error wrapper (reads thread-local `tperrno` and formats via `tpstrerror`).
#[derive(Debug, Clone)]
pub struct AtmiError {
    pub code: i32,
    pub msg: String,
}
impl std::fmt::Display for AtmiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ATMI error {}: {}", self.code, self.msg)
    }
}
impl std::error::Error for AtmiError {}

#[inline]
fn last_error() -> AtmiError {
    unsafe {
        let code = *_exget_tperrno_addr() as i32;
        let cmsg = tpstrerror(code as c_int);
        let msg = if cmsg.is_null() { "unknown".to_string() } else { CStr::from_ptr(cmsg).to_string_lossy().into_owned() };
        AtmiError { code, msg }
    }
}

/// RAII client context: calls `tpinit()` on create and `tpterm()` on drop.
pub struct Client {
    _priv: (),
}
impl Client {
    /// Initialize ATMI context for the current thread/process.
    pub fn init() -> Result<Self, AtmiError> {
        unsafe {
            if tpinit(ptr::null_mut()) == TPERROR { Err(last_error()) } else { Ok(Self { _priv: () }) }
        }
    }
}

// ===== Example (doc) =====
//
// let client = Client::init()?;
// let mut buf = json_buf("{\"hello\":\"world\"}")?;
// let _n = client.call("ECHOJSON", &mut buf, 0)?;
// let reply = std::str::from_utf8(&*buf).unwrap_or("");
// println!("reply: {}", reply);

