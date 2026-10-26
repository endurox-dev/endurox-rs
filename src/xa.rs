//! Named XA resource-manager operations.
use crate::{raw, AtmiCtx, AtmiError, AtmiResult};
use std::ffi::CString;
use std::os::raw::{c_long, c_void};

impl AtmiCtx {
    /// Open automatic resource managers, or all configured RMs with `TPTXOPENALL`.
    pub fn tpopen2(&self, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpopen2(flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpopen2(self.c_ctx_ptr(), flags) };
        self.rc_to_result(rc)
    }

    /// Open the configured resource manager `name`.
    /// `flags` is reserved; use zero. Manual RMs require explicit start/end calls.
    pub fn tprmopen(&self, name: &str, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let name = CString::new(name)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "RM name contains NUL"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tprmopen(name.as_ptr(), flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otprmopen(self.c_ctx_ptr(), name.as_ptr(), flags) };
        self.rc_to_result(rc)
    }

    /// Close the configured resource manager `name`.
    /// `flags` is reserved; use zero. Manual RMs require explicit start/end calls.
    pub fn tprmclose(&self, name: &str, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let name = CString::new(name)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "RM name contains NUL"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tprmclose(name.as_ptr(), flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otprmclose(self.c_ctx_ptr(), name.as_ptr(), flags) };
        self.rc_to_result(rc)
    }

    /// Associate the current transaction with the configured resource manager `name`.
    /// `flags` is reserved; use zero. Manual RMs require explicit start/end calls.
    pub fn tprmstart(&self, name: &str, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let name = CString::new(name)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "RM name contains NUL"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tprmstart(name.as_ptr(), flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otprmstart(self.c_ctx_ptr(), name.as_ptr(), flags) };
        self.rc_to_result(rc)
    }

    /// End the current transaction association with the configured resource manager `name`.
    /// `flags` is reserved; use zero. Manual RMs require explicit start/end calls.
    pub fn tprmend(&self, name: &str, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let name = CString::new(name)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "RM name contains NUL"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tprmend(name.as_ptr(), flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otprmend(self.c_ctx_ptr(), name.as_ptr(), flags) };
        self.rc_to_result(rc)
    }

    /// Borrow the provider-owned connection for the configured numeric `rmid`.
    /// Closing the RM invalidates it. Never free this pointer.
    ///
    /// # Safety
    /// Any use must obey the provider's type, lifetime and thread restrictions.
    pub unsafe fn tpgetconnrm(&self, rmid: i32) -> AtmiResult<*mut c_void> {
        #[cfg(not(feature = "ctx-send"))]
        let pointer = unsafe { raw::tpgetconnrm(rmid) };
        #[cfg(feature = "ctx-send")]
        let pointer = unsafe { raw::Otpgetconnrm(self.c_ctx_ptr(), rmid) };
        if pointer.is_null() {
            Err(self.atmi_last_error())
        } else {
            Ok(pointer)
        }
    }
}
