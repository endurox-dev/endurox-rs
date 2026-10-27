//! Saga participants, recovery metadata and global transaction identifiers.
//!
//! [`TpSagaCtl`] owns one native control record and its payload allocation.
//! Callbacks must tolerate redelivery. Global [`Xid`] strings, branch identifiers
//! and suspended [`TpTranId`] tokens are different identifiers.
#![allow(clippy::unnecessary_cast)]
use crate::{raw, AtmiCtx, AtmiError, AtmiResult, TpTranId};
use std::ffi::CString;
use std::os::raw::{c_char, c_long};

/// Native XA global identifier, obtained from Saga registration or its string codec.
/// Its binary layout is private; use [`AtmiCtx::tpxidtostring`] for transport.
#[derive(Clone, Debug)]
pub struct Xid(pub(crate) raw::XID);

/// Saga registration and participant metadata. Empty URLs omit their callbacks.
///
/// The payload is owned by this value and remains at a stable address when the
/// control moves. Native calls borrow it synchronously; no pointers escape to
/// the coordinator. Lookup and URL-only updates preserve this local payload.
pub struct TpSagaCtl {
    inner: raw::TPSAGACTL,
    payload: Box<[u8]>,
}

impl Default for TpSagaCtl {
    /// Create an empty registration; select a configured Saga and callback URLs.
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
            payload: Box::default(),
        }
    }
}

impl TpSagaCtl {
    /// Return registration mode flags; distinct from each operation's flags.
    pub fn flags(&self) -> i64 {
        self.inner.flags as i64
    }

    /// Return the first-registration lifetime in seconds; zero adds no limit.
    pub fn timeout(&self) -> i64 {
        self.inner.timeout as i64
    }

    /// Set the optional registration lifetime in seconds. It shortens the
    /// global deadline and bounds renewal; metadata updates retain the limit.
    /// Negative values fail without changing the control.
    pub fn set_timeout(&mut self, seconds: i64) -> AtmiResult<&mut Self> {
        let timeout = c_long::try_from(seconds)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "timeout exceeds native long"))?;
        if timeout < 0 {
            return Err(AtmiError::new(raw::TPEINVAL, "negative Saga timeout"));
        }
        self.inner.timeout = timeout;
        Ok(self)
    }

    /// Set registration mode flags (`0` or `TPSAGA_NOLOCK`).
    pub fn set_flags(&mut self, flags: i64) -> AtmiResult<&mut Self> {
        self.inner.flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "Saga flags exceed native long"))?;
        Ok(self)
    }

    /// Borrow the registered payload bytes. An empty slice omits the payload.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Copy `payload` into owned stable storage; reject more than 4096 bytes.
    /// Validation failure preserves the previous payload. Empty clears it.
    pub fn set_payload(&mut self, payload: &[u8]) -> AtmiResult<&mut Self> {
        if payload.len() > raw::NDRX_SAGA_PAYLOAD_MAX as usize {
            return Err(AtmiError::new(
                raw::TPEINVAL,
                "Saga payload exceeds native limit",
            ));
        }
        self.payload = payload.into();
        self.inner.payload = if self.payload.is_empty() {
            std::ptr::null_mut()
        } else {
            self.payload.as_mut_ptr().cast()
        };
        self.inner.payload_len = self.payload.len() as c_long;
        Ok(self)
    }

    /// Copy the native global XID returned by registration.
    pub fn xid(&self) -> Xid {
        Xid(self.inner.xid)
    }

    /// Select `xid` for registration with `TPSAGA_JOINXID`.
    pub fn set_xid(&mut self, xid: &Xid) -> &mut Self {
        self.inner.xid = xid.0;
        self
    }

    /// Return the configured Saga profile name.
    pub fn saga_name(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.saga_name)
    }

    /// Set the configured Saga profile name; reject embedded NULs and values beyond the native limit.
    pub fn set_saga_name(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.saga_name, value, "saga_name")?;
        Ok(self)
    }

    /// Return the participant idempotency key.
    pub fn saga_key(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.saga_key)
    }

    /// Set the participant idempotency key; reject embedded NULs and values beyond the native limit.
    pub fn set_saga_key(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.saga_key, value, "saga_key")?;
        Ok(self)
    }

    /// Return the payload MIME type.
    pub fn content_type(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.content_type)
    }

    /// Set the payload MIME type; reject embedded NULs and values beyond the native limit.
    pub fn set_content_type(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.content_type, value, "content_type")?;
        Ok(self)
    }

    /// Return the modified Base64 branch identifier.
    pub fn branch_xid(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.branch_xid)
    }

    /// Set the modified Base64 branch identifier; reject embedded NULs and values beyond the native limit.
    pub fn set_branch_xid(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.branch_xid, value, "branch_xid")?;
        Ok(self)
    }

    /// Return the commit callback URL.
    pub fn commit_url(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.commit_url)
    }

    /// Set the commit callback URL; reject embedded NULs and values beyond the native limit.
    pub fn set_commit_url(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.commit_url, value, "commit_url")?;
        Ok(self)
    }

    /// Return the compensation callback URL.
    pub fn rollback_url(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.rollback_url)
    }

    /// Set the compensation callback URL; reject embedded NULs and values beyond the native limit.
    pub fn set_rollback_url(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.rollback_url, value, "rollback_url")?;
        Ok(self)
    }

    /// Return the asynchronous status URL.
    pub fn status_url(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.status_url)
    }

    /// Set the asynchronous status URL; reject embedded NULs and values beyond the native limit.
    pub fn set_status_url(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.status_url, value, "status_url")?;
        Ok(self)
    }

    /// Return the forget callback URL.
    pub fn forget_url(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.forget_url)
    }

    /// Set the forget callback URL; reject embedded NULs and values beyond the native limit.
    pub fn set_forget_url(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.forget_url, value, "forget_url")?;
        Ok(self)
    }

    /// Return the final outcome notification URL.
    pub fn after_url(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.after_url)
    }

    /// Set the final outcome notification URL; reject embedded NULs and values beyond the native limit.
    pub fn set_after_url(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.after_url, value, "after_url")?;
        Ok(self)
    }
}

/// Transaction lookup/renewal record, stored directly in native `TPTXSTATUS`.
/// Select exactly one identifier with `TPTXS_XID`, `TPTXS_BRXID` or `TPTXS_TRANID`.
pub struct TpTxStatus {
    inner: raw::TPTXSTATUS,
}

impl Default for TpTxStatus {
    /// Create a cleared query; its identifier and flags must be set before use.
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

impl TpTxStatus {
    /// Return the selected identifier flags.
    pub fn flags(&self) -> i64 {
        self.inner.flags as i64
    }
    /// Select which identifier field the coordinator uses.
    pub fn set_flags(&mut self, flags: i64) -> AtmiResult<&mut Self> {
        self.inner.flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "status flags exceed native long"))?;
        Ok(self)
    }
    /// Copy the global XID query field.
    pub fn xid(&self) -> Xid {
        Xid(self.inner.xid)
    }
    /// Set the identifier used with `TPTXS_XID`.
    pub fn set_xid(&mut self, xid: &Xid) -> &mut Self {
        self.inner.xid = xid.0;
        self
    }
    /// Return the branch identifier query field.
    pub fn branch_xid(&self) -> String {
        crate::fixed_cstr_to_string(&self.inner.branch_xid)
    }
    /// Set the branch identifier used with `TPTXS_BRXID`, validating its length and NULs.
    pub fn set_branch_xid(&mut self, value: &str) -> AtmiResult<&mut Self> {
        crate::write_fixed_str(&mut self.inner.branch_xid, value, "branch_xid")?;
        Ok(self)
    }
    /// Copy the suspended transaction query field.
    pub fn tranid(&self) -> TpTranId {
        TpTranId(self.inner.tranid)
    }
    /// Set the identifier used with `TPTXS_TRANID`.
    pub fn set_tranid(&mut self, id: &TpTranId) -> &mut Self {
        self.inner.tranid = id.0;
        self
    }
    /// Return the coordinator's `TPTXS_STATUS_*` result.
    pub fn status(&self) -> i64 {
        self.inner.status as i64
    }
    /// Return the total timeout in seconds, or the requested renewal before the call.
    pub fn timeout(&self) -> i64 {
        self.inner.timeout as i64
    }
    /// Request a positive timeout in seconds from now for `tptxrenew`.
    pub fn set_timeout(&mut self, seconds: i64) -> AtmiResult<&mut Self> {
        let timeout = c_long::try_from(seconds)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "timeout exceeds native long"))?;
        if timeout <= 0 {
            return Err(AtmiError::new(raw::TPEINVAL, "timeout must be positive"));
        }
        self.inner.timeout = timeout;
        Ok(self)
    }
    /// Return the coordinator's remaining timeout in seconds.
    pub fn timeout_left(&self) -> i64 {
        self.inner.timeout_left as i64
    }
}

impl AtmiCtx {
    /// Register a participant and fill its global and branch identifiers.
    /// A second registration on the same branch fails; `TPSAGA_UPDATE` explicitly
    /// replaces a participant with the same key. End the working section with `tpsagaend`.
    ///
    /// - `ctl`: Native control record containing the participant or transaction selector.
    /// - `flags`: `TPSAGA_JOINXID`, `TPSAGA_UPDATE`, or zero.
    pub fn tpsagaenlist(&self, ctl: &mut TpSagaCtl, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsagaenlist(&mut ctl.inner, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsagaenlist(self.c_ctx_ptr(), &mut ctl.inner, flags) };
        self.rc_to_result(rc)
    }

    /// End local work while retaining the participant registration.
    /// With `dlmspace` configured, releases the lock supplied by the Distributed
    /// Lock Manager (DLM) in Enduro/X Enterprise Edition (`endurox-ee`).
    /// Standard mode requires this call even without DLM; `TPSAGA_NOLOCK`
    /// registrations have no section to end.
    ///
    /// - `ctl`: Native control record containing the participant or transaction selector.
    /// - `flags`: reserved; use zero.
    pub fn tpsagaend(&self, ctl: &mut TpSagaCtl, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsagaend(&mut ctl.inner, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsagaend(self.c_ctx_ptr(), &mut ctl.inner, flags) };
        self.rc_to_result(rc)
    }

    /// Read participant metadata and URLs by `ctl.branch_xid()`.
    /// The locally owned payload is preserved; the coordinator does not return payloads.
    ///
    /// - `ctl`: Native control record containing the participant or transaction selector.
    /// - `flags`: reserved; use zero.
    pub fn tpsagaget(&self, ctl: &mut TpSagaCtl, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsagaget(&mut ctl.inner, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsagaget(self.c_ctx_ptr(), &mut ctl.inner, flags) };
        self.rc_to_result(rc)
    }

    /// Replace participant metadata while active. `TPSAGA_URLSONLY` also permits
    /// endpoint repair during recovery and preserves the coordinator's payload.
    ///
    /// - `ctl`: Native control record containing the participant or transaction selector.
    /// - `flags`: `TPSAGA_URLSONLY` or zero.
    pub fn tpsagaupdate(&self, ctl: &mut TpSagaCtl, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsagaupdate(&mut ctl.inner, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsagaupdate(self.c_ctx_ptr(), &mut ctl.inner, flags) };
        self.rc_to_result(rc)
    }

    /// Withdraw a participant while its transaction is active, preventing callbacks.
    /// This does not end its local working section; call `tpsagaend` for that.
    ///
    /// - `ctl`: Native control record containing the participant or transaction selector.
    /// - `flags`: reserved; use zero.
    pub fn tpsagaleave(&self, ctl: &mut TpSagaCtl, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsagaleave(&mut ctl.inner, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsagaleave(self.c_ctx_ptr(), &mut ctl.inner, flags) };
        self.rc_to_result(rc)
    }

    /// Query the coordinator and fill status and timeout fields.
    /// A completed or unknown transaction returns `TPTXS_STATUS_NOTFOUND`.
    ///
    /// - `ctl`: Native control record containing the participant or transaction selector.
    /// - `flags`: reserved; use zero.
    pub fn tptxstatus(&self, ctl: &mut TpTxStatus, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptxstatus(&mut ctl.inner, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptxstatus(self.c_ctx_ptr(), &mut ctl.inner, flags) };
        self.rc_to_result(rc)
    }

    /// Renew using `ctl.timeout()` seconds from now, capped by the earliest
    /// registered participant deadline.
    /// On success the coordinator replaces the status and timeout fields.
    ///
    /// - `ctl`: Native control record containing the participant or transaction selector.
    /// - `flags`: reserved; use zero.
    pub fn tptxrenew(&self, ctl: &mut TpTxStatus, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptxrenew(&mut ctl.inner, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptxrenew(self.c_ctx_ptr(), &mut ctl.inner, flags) };
        self.rc_to_result(rc)
    }

    /// Encode `xid` using the native global XID codec, distinct from a branch token.
    pub fn tpxidtostring(&self, xid: &Xid) -> AtmiResult<String> {
        let mut value = [0 as c_char; raw::TPCONVMAXSTR as usize];
        let mut xid = xid.0;
        let flags = (raw::TPCONVXID | raw::TPTOSTRING) as c_long;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpconvert(
                value.as_mut_ptr(),
                (&mut xid as *mut raw::XID).cast(),
                flags,
            )
        };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpconvert(
                self.c_ctx_ptr(),
                value.as_mut_ptr(),
                (&mut xid as *mut raw::XID).cast(),
                flags,
            )
        };
        self.rc_to_result(rc)?;
        Ok(crate::fixed_cstr_to_string(&value))
    }

    /// Decode `value` as a native global XID. Reject malformed text and embedded NULs.
    pub fn tpxidfromstring(&self, value: &str) -> AtmiResult<Xid> {
        let value =
            CString::new(value).map_err(|_| AtmiError::new(raw::TPEINVAL, "XID contains NUL"))?;
        let mut xid: raw::XID = unsafe { std::mem::zeroed() };
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpconvert(
                value.as_ptr() as *mut c_char,
                (&mut xid as *mut raw::XID).cast(),
                raw::TPCONVXID as c_long,
            )
        };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpconvert(
                self.c_ctx_ptr(),
                value.as_ptr() as *mut c_char,
                (&mut xid as *mut raw::XID).cast(),
                raw::TPCONVXID as c_long,
            )
        };
        self.rc_to_result(rc)?;
        Ok(Xid(xid))
    }

    /// Convert a global XID string to a participant token for `tpresume`.
    /// The token has no initiator rights: `tpcommit` and `tpabort` fail with
    /// `TPEPROTO`. Detach with `tpsuspend`, with `TPTXABORTONLY` if the work
    /// failed. Conversion is local and does not check the transaction state.
    ///
    /// # Arguments
    ///
    /// - `xid`: Global XID string (`tpxidtostring`), not a branch XID or LRA URL.
    pub fn tptranidfromxid(&self, xid: &str) -> AtmiResult<TpTranId> {
        let value =
            CString::new(xid).map_err(|_| AtmiError::new(raw::TPEINVAL, "XID contains NUL"))?;
        let mut tranid: raw::TPTRANID = unsafe { std::mem::zeroed() };
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpconvert(
                value.as_ptr() as *mut c_char,
                (&mut tranid as *mut raw::TPTRANID).cast(),
                raw::TPCONVXIDTRAN as c_long,
            )
        };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpconvert(
                self.c_ctx_ptr(),
                value.as_ptr() as *mut c_char,
                (&mut tranid as *mut raw::TPTRANID).cast(),
                raw::TPCONVXIDTRAN as c_long,
            )
        };
        self.rc_to_result(rc)?;
        Ok(TpTranId(tranid))
    }
}
