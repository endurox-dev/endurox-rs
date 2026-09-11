//! Rust bindings for Enduro/X contexts, typed buffers, services, queues, and scripting.
//!
//! Start with [`AtmiCtx`]; buffers borrow their context and use native Enduro/X allocation.
//! Operation flags and subsystem-specific errors are re-exported at the crate root.
//!
//! # Scripting
//!
//! Start with the [scripting guide](TpScrVm) for setup, a complete Python example,
//! Rust callbacks, error propagation, bytecode, and cleanup. [`TpScrBuffers`]
//! explains the parameter/input/output slots, aliases, and ownership transfer.
//! Create a VM with [`AtmiCtx::tpscrinit`]; its methods retain the native `tpscr*` names.
#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
use std::ffi::CStr;

pub use endurox_rs_derive::{UbfDeserialize, UbfSerialize, ViewDeserialize, ViewSerialize};

pub(crate) mod raw {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[doc(hidden)]
pub mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/test.rs"));
}

#[cfg(feature = "async")]
mod async_atmi;
mod atmictx;
mod atmictx_log;
mod atmictx_srv;
mod atmictx_ubf;
mod atmictx_xatmi;
pub mod dlm;
mod errors;
mod flags;
mod nstdutil;
mod script_buffers;
mod tpscript;
mod tpsvcinfo;
mod typed_buf;
mod typed_ubf;
mod typed_view;
mod types;
mod ubf_complex;
mod ubf_search;
mod ubf_serde;
mod view_serde;

#[cfg(feature = "async")]
pub use async_atmi::{AsyncAtmiCtx, AsyncReplyDriver};
#[cfg(feature = "async-io")]
pub use async_atmi::{AsyncIoAtmiCtx, AsyncIoReplyDriver};
#[cfg(feature = "tokio")]
pub use async_atmi::{TokioAtmiCtx, TokioReplyDriver};
pub use atmictx::AtmiCtx;
pub use atmictx_log::LogLevel;
pub use atmictx_srv::{
    PollerEvent, RustBeforePollCallback, RustPeriodCallback, RustPollerCallback,
    RustServerDoneHook, RustServerInitHook, RustServerThreadDoneHook, RustServerThreadInitHook,
    RustServiceCallback, ServerHooks, TpReturnStatus,
};
pub use atmictx_ubf::{BFldLocInfo, UbfExprCallback, UbfExprCallback2, UbfExprTree, UbfFieldType};
pub use dlm::{TpDlmCtl, TpDlmTimeout};
pub use errors::{AtmiError, AtmiResult, NstdError, NstdResult, UbfError, UbfResult};
pub use flags::{
    TPABSOLUTE, TPBLK_ALL, TPBLK_NEXT, TPCONV, TPEX_STRING, TPGETANY, TPNOBLOCK, TPNOCHANGE,
    TPNOREPLY, TPNOTIME, TPNOTRAN, TPRECVONLY, TPSENDONLY, TPSIGRSTRT, TPTRAN, TPTRANSUSPEND,
};
pub use nstdutil::NdrxStdCfgStr;
pub use script_buffers::{TpScrBuffers, TpScrSlot};
pub use tpscript::{
    TpScrBytecode, TpScrCallback, TpScrCallbackContext, TpScrCfg, TpScrError, TpScrResult, TpScrVm,
};
/// Load a script unit as a package that may contain submodules.
pub const NDRX_TPSCR_PACKAGE: i64 = raw::NDRX_TPSCR_PACKAGE as i64;
/// Replace or reload an existing script registration.
pub const NDRX_TPSCR_REPLACE: i64 = raw::NDRX_TPSCR_REPLACE as i64;
/// Treat a loaded script name as literal text instead of splitting it at dots.
pub const NDRX_TPSCR_FLAT: i64 = raw::NDRX_TPSCR_FLAT as i64;
pub use tpsvcinfo::TpSvcInfo;
pub use typed_buf::{TpTypeInfo, TypedBuffer};
pub use typed_ubf::{
    BorrowedBuffer, BorrowedUbf, FastAdder, IntoUbfValue, TypedUbf, UbfField, UbfGetValue,
    UbfIterator, UbfValue,
};
pub use typed_view::{BvNextState, IntoViewValue, TypedView, ViewValue, BVACCESS_NOTNULL};
pub use types::{ClientId, TpTranId};
pub use ubf_search::{BorrowedView, UbfFieldRef};
#[doc(hidden)]
pub use ubf_serde::{ubf_group_clear, ubf_group_field_check, ubf_mapping_present, ubf_occurrence};
pub use ubf_serde::{
    ubf_read_adhoc, ubf_read_nested, ubf_read_ptr, ubf_write_adhoc, ubf_write_nested,
    ubf_write_ptr, EmbeddedUbf, EmbeddedView, PointerUbf, PointerView, UbfAdhoc, UbfCarray,
    UbfDeserialize, UbfFieldDeserialize, UbfFieldSerialize, UbfGroupDeserialize,
    UbfGroupFieldDeserialize, UbfGroupFieldSerialize, UbfGroupSerialize, UbfMappedDeserialize,
    UbfMappedSerialize, UbfMapping, UbfSerialize,
};
#[doc(hidden)]
pub use view_serde::check_view;
pub use view_serde::{ViewDeserialize, ViewFieldDeserialize, ViewFieldSerialize, ViewSerialize};

pub const TPQCORRID: i64 = raw::TPQCORRID as i64;
pub const TPQFAILUREQ: i64 = raw::TPQFAILUREQ as i64;
pub const TPQBEFOREMSGID: i64 = raw::TPQBEFOREMSGID as i64;
pub const TPQGETBYMSGIDOLD: i64 = raw::TPQGETBYMSGIDOLD as i64;
pub const TPQMSGID: i64 = raw::TPQMSGID as i64;
pub const TPQPRIORITY: i64 = raw::TPQPRIORITY as i64;
pub const TPQTOP: i64 = raw::TPQTOP as i64;
pub const TPQWAIT: i64 = raw::TPQWAIT as i64;
pub const TPQREPLYQ: i64 = raw::TPQREPLYQ as i64;
pub const TPQTIME_ABS: i64 = raw::TPQTIME_ABS as i64;
pub const TPQTIME_REL: i64 = raw::TPQTIME_REL as i64;
pub const TPQGETBYCORRIDOLD: i64 = raw::TPQGETBYCORRIDOLD as i64;
pub const TPQPEEK: i64 = raw::TPQPEEK as i64;
pub const TPQDELIVERYQOS: i64 = raw::TPQDELIVERYQOS as i64;
pub const TPQREPLYQOS: i64 = raw::TPQREPLYQOS as i64;
pub const TPQEXPTIME_ABS: i64 = raw::TPQEXPTIME_ABS as i64;
pub const TPQEXPTIME_REL: i64 = raw::TPQEXPTIME_REL as i64;
pub const TPQEXPTIME_NONE: i64 = raw::TPQEXPTIME_NONE as i64;
pub const TPQGETBYMSGID: i64 = raw::TPQGETBYMSGID as i64;
pub const TPQGETBYCORRID: i64 = raw::TPQGETBYCORRID as i64;
pub const TPQASYNC: i64 = raw::TPQASYNC as i64;
pub const TPQKEEPORIG: i64 = raw::TPQKEEPORIG as i64;
pub const TPQQOSDEFAULTPERSIST: i64 = raw::TPQQOSDEFAULTPERSIST as i64;
pub const TPQQOSPERSISTENT: i64 = raw::TPQQOSPERSISTENT as i64;
pub const TPQQOSNONPERSISTENT: i64 = raw::TPQQOSNONPERSISTENT as i64;

/// Event subscription control block used by [`AtmiCtx::tpsubscribe`].
///
/// This is an opaque Rust wrapper around the Enduro/X `TPEVCTL` structure.
/// Set `name1` to the destination service and flags to [`TPEVSERVICE`].
/// `name2` is reserved by the native event broker.
pub struct TpEvCtl {
    inner: raw::TPEVCTL,
}

/// Cleared native initialization for `TpEvCtl`.
impl Default for TpEvCtl {
    /// Create an event control block with all fields and flags cleared.
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

/// Native access to an event subscription control block.
impl TpEvCtl {
    /// Return the event-delivery flags.
    pub fn flags(&self) -> i64 {
        self.inner.flags as i64
    }

    /// Set event-delivery flags, including `TPEVSERVICE` and optionally `TPEVPERSIST`.
    pub fn set_flags(&mut self, flags: i64) -> &mut Self {
        self.inner.flags = flags as _;
        self
    }

    /// Return the destination service name.
    pub fn name1(&self) -> String {
        fixed_cstr_to_string(&self.inner.name1)
    }

    /// Set the destination service name; reject embedded NULs and oversized names.
    pub fn set_name1(&mut self, name: &str) -> AtmiResult<&mut Self> {
        write_fixed_str(&mut self.inner.name1, name, "name1")?;
        Ok(self)
    }

    /// Return the reserved second name.
    pub fn name2(&self) -> String {
        fixed_cstr_to_string(&self.inner.name2)
    }

    /// Set the reserved second name; reject embedded NULs and oversized names.
    pub fn set_name2(&mut self, name: &str) -> AtmiResult<&mut Self> {
        write_fixed_str(&mut self.inner.name2, name, "name2")?;
        Ok(self)
    }
}

/// Deliver matching events to the service in [`TpEvCtl::name1`].
pub const TPEVSERVICE: i64 = raw::TPEVSERVICE as i64;
/// Keep a subscription when its destination service becomes unavailable.
pub const TPEVPERSIST: i64 = raw::TPEVPERSIST as i64;

/// Persistent queue control block used by queue enqueue/dequeue APIs.
///
/// Flags are explicit, matching Enduro/X `TPQCTL.flags`. Use setters for the
/// fixed-size fields so Rust performs length/NUL validation before C sees them.
pub struct TpQCtl {
    inner: raw::TPQCTL,
}

/// Cleared native initialization for `TpQCtl`.
impl Default for TpQCtl {
    /// Create a queue control block with all fields and flags cleared.
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

/// Queue-control flags, message identifiers, delivery options, and diagnostics.
impl TpQCtl {
    /// Borrow the native control block for an Enduro/X call.
    #[inline]
    pub(crate) fn as_mut_ptr(&mut self) -> *mut raw::TPQCTL {
        &mut self.inner
    }

    /// Return the queue-control flags that select which fields Enduro/X uses.
    pub fn flags(&self) -> i64 {
        self.inner.flags as i64
    }

    /// Replace all queue-control flags and return this control block.
    ///
    /// # Arguments
    ///
    /// - `flags`: Bitmask of `TPQ*` control flags to replace, enable, or disable.
    pub fn set_flags(&mut self, flags: i64) -> &mut Self {
        self.inner.flags = flags as _;
        self
    }

    /// Enable additional queue-control flags and return this control block.
    ///
    /// # Arguments
    ///
    /// - `flags`: Bitmask of `TPQ*` control flags to replace, enable, or disable.
    pub fn add_flags(&mut self, flags: i64) -> &mut Self {
        self.inner.flags |= flags as std::os::raw::c_long;
        self
    }

    /// Disable selected queue-control flags and return this control block.
    ///
    /// # Arguments
    ///
    /// - `flags`: Bitmask of `TPQ*` control flags to replace, enable, or disable.
    pub fn clear_flags(&mut self, flags: i64) -> &mut Self {
        self.inner.flags &= !(flags as std::os::raw::c_long);
        self
    }

    /// Return the configured time at which the message becomes available.
    pub fn deq_time(&self) -> i64 {
        self.inner.deq_time as i64
    }

    /// Set message availability time; select its interpretation with `TPQTIME_ABS` or
    /// `TPQTIME_REL`.
    ///
    /// # Arguments
    ///
    /// - `deq_time`: Unix timestamp in seconds for `TPQTIME_ABS`, or a delay in seconds for
    ///   `TPQTIME_REL`.
    pub fn set_deq_time(&mut self, deq_time: i64) -> &mut Self {
        self.inner.deq_time = deq_time as _;
        self
    }

    /// Return the configured message priority.
    pub fn priority(&self) -> i64 {
        self.inner.priority as i64
    }

    /// Set message priority; enable `TPQPRIORITY` to apply it when enqueueing.
    ///
    /// # Arguments
    ///
    /// - `priority`: Message priority, from 1 (lowest) to 100 (highest).
    pub fn set_priority(&mut self, priority: i64) -> &mut Self {
        self.inner.priority = priority as _;
        self
    }

    /// Return the queue-specific diagnostic code, typically after `TPEDIAGNOSTIC`.
    pub fn diagnostic(&self) -> i64 {
        self.inner.diagnostic as i64
    }

    /// Return the queue diagnostic text, replacing invalid UTF-8 sequences.
    pub fn diagmsg(&self) -> String {
        fixed_cstr_to_string(&self.inner.diagmsg)
    }

    /// Borrow the stored message identifier up to its first zero byte.
    pub fn msgid(&self) -> &[u8] {
        fixed_bytes_until_nul(&self.inner.msgid)
    }

    /// Copy a message identifier into the control block, zero-filling the remaining storage.
    ///
    /// # Arguments
    ///
    /// - `msgid`: Identifier bytes; must leave room for one trailing zero byte in the native field.
    pub fn set_msgid(&mut self, msgid: &[u8]) -> AtmiResult<&mut Self> {
        write_fixed_bytes(&mut self.inner.msgid, msgid, "msgid")?;
        Ok(self)
    }

    /// Borrow the stored correlation identifier up to its first zero byte.
    pub fn corrid(&self) -> &[u8] {
        fixed_bytes_until_nul(&self.inner.corrid)
    }

    /// Copy a correlation identifier into the control block, zero-filling the remaining storage.
    ///
    /// # Arguments
    ///
    /// - `corrid`: Correlation bytes; must leave room for one trailing zero byte in the native
    ///   field.
    pub fn set_corrid(&mut self, corrid: &[u8]) -> AtmiResult<&mut Self> {
        write_fixed_bytes(&mut self.inner.corrid, corrid, "corrid")?;
        Ok(self)
    }

    /// Return the queue name configured for replies.
    pub fn reply_queue(&self) -> String {
        fixed_cstr_to_string(&self.inner.replyqueue)
    }

    /// Set the reply queue name; enable `TPQREPLYQ` to apply it.
    ///
    /// # Arguments
    ///
    /// - `replyqueue`: Reply queue name without embedded NUL bytes; must fit the native field.
    pub fn set_reply_queue(&mut self, replyqueue: &str) -> AtmiResult<&mut Self> {
        write_fixed_str(&mut self.inner.replyqueue, replyqueue, "replyqueue")?;
        Ok(self)
    }

    /// Return the queue name configured for failed messages.
    pub fn failure_queue(&self) -> String {
        fixed_cstr_to_string(&self.inner.failurequeue)
    }

    /// Set the failure queue name; enable `TPQFAILUREQ` to apply it.
    ///
    /// # Arguments
    ///
    /// - `failurequeue`: Failure queue name without embedded NUL bytes; must fit the native field.
    pub fn set_failure_queue(&mut self, failurequeue: &str) -> AtmiResult<&mut Self> {
        write_fixed_str(&mut self.inner.failurequeue, failurequeue, "failurequeue")?;
        Ok(self)
    }

    /// Return the application-defined return code stored with the message.
    pub fn urcode(&self) -> i64 {
        self.inner.urcode as i64
    }

    /// Set the application-defined return code stored with the message.
    ///
    /// # Arguments
    ///
    /// - `urcode`: Application-defined return code to store.
    pub fn set_urcode(&mut self, urcode: i64) -> &mut Self {
        self.inner.urcode = urcode as _;
        self
    }

    /// Return the application authentication key stored in the control block.
    pub fn appkey(&self) -> i64 {
        self.inner.appkey as i64
    }

    /// Set the application authentication key in the control block.
    ///
    /// # Arguments
    ///
    /// - `appkey`: Application authentication key to store.
    pub fn set_appkey(&mut self, appkey: i64) -> &mut Self {
        self.inner.appkey = appkey as _;
        self
    }

    /// Return the delivery persistence setting stored in the control block.
    pub fn delivery_qos(&self) -> i64 {
        self.inner.delivery_qos as i64
    }

    /// Set delivery persistence; enable `TPQDELIVERYQOS` to apply it.
    ///
    /// # Arguments
    ///
    /// - `delivery_qos`: A `TPQQOS*` persistence value for delivery.
    pub fn set_delivery_qos(&mut self, delivery_qos: i64) -> &mut Self {
        self.inner.delivery_qos = delivery_qos as _;
        self
    }

    /// Return the reply persistence setting stored in the control block.
    pub fn reply_qos(&self) -> i64 {
        self.inner.reply_qos as i64
    }

    /// Set reply persistence; enable `TPQREPLYQOS` to apply it.
    ///
    /// # Arguments
    ///
    /// - `reply_qos`: A `TPQQOS*` persistence value for replies.
    pub fn set_reply_qos(&mut self, reply_qos: i64) -> &mut Self {
        self.inner.reply_qos = reply_qos as _;
        self
    }

    /// Return the configured message expiration time.
    pub fn exp_time(&self) -> i64 {
        self.inner.exp_time as i64
    }

    /// Set message expiration time; select its interpretation with the `TPQEXPTIME_*` flags.
    ///
    /// # Arguments
    ///
    /// - `exp_time`: Unix timestamp or lifetime in seconds, as selected by `TPQEXPTIME_ABS` or
    ///   `TPQEXPTIME_REL`.
    pub fn set_exp_time(&mut self, exp_time: i64) -> &mut Self {
        self.inner.exp_time = exp_time as _;
        self
    }
}

/// Validate and copy a string into a fixed C field, reserving a trailing NUL.
///
/// # Arguments
///
/// - `dst`: Destination C field, including space for a trailing NUL.
/// - `value`: Contents to copy; must be shorter than the destination field.
/// - `field`: Field name to include in validation errors.
fn write_fixed_str(dst: &mut [std::os::raw::c_char], value: &str, field: &str) -> AtmiResult<()> {
    if value.as_bytes().contains(&0) {
        return Err(AtmiError::new(
            raw::TPEINVAL,
            format!("{field} contains NUL byte"),
        ));
    }
    write_fixed_bytes(dst, value.as_bytes(), field)
}

/// Copy bytes into a fixed C field, reserving and zero-filling unused storage.
///
/// # Arguments
///
/// - `dst`: Destination C field, including space for a trailing NUL.
/// - `value`: Contents to copy; must be shorter than the destination field.
/// - `field`: Field name to include in validation errors.
fn write_fixed_bytes(
    dst: &mut [std::os::raw::c_char],
    value: &[u8],
    field: &str,
) -> AtmiResult<()> {
    if value.len() >= dst.len() {
        return Err(AtmiError::new(
            raw::TPEINVAL,
            format!("{field} too long: max {} bytes", dst.len() - 1),
        ));
    }
    dst.fill(0);
    for (out, byte) in dst.iter_mut().zip(value.iter().copied()) {
        *out = byte as std::os::raw::c_char;
    }
    Ok(())
}

/// Borrow a fixed C field as bytes, stopping at the first NUL or the slice end.
///
/// # Arguments
///
/// - `src`: Source character array; a missing NUL uses the entire slice.
fn fixed_bytes_until_nul(src: &[std::os::raw::c_char]) -> &[u8] {
    let len = src.iter().position(|&b| b == 0).unwrap_or(src.len());
    unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, len) }
}

/// Copy a NUL-terminated C field into a string, replacing invalid UTF-8.
///
/// # Arguments
///
/// - `src`: Source C field; string conversion requires a terminating NUL within it.
fn fixed_cstr_to_string(src: &[std::os::raw::c_char]) -> String {
    if src.first().copied().unwrap_or_default() == 0 {
        String::new()
    } else {
        unsafe { CStr::from_ptr(src.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    }
}
