//! W3C propagation, application spans and persistent context tags.
use crate::{raw, AtmiCtx, AtmiError, AtmiResult, TypedUbf};
use std::ffi::CString;
use std::os::raw::{c_char, c_long};

/// Span started by [`AtmiCtx::tptracespanbegin`]. It is valid only in the
/// context that started it, until [`AtmiCtx::tptracespanend`] succeeds.
#[must_use = "end the span with tptracespanend"]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TpTraceSpan(usize);

impl AtmiCtx {
    /// Import `value` as W3C traceparent; an empty value clears propagation.
    /// `flags` is reserved and must be zero. Native code copies `value`.
    pub fn tptracecontextset(&self, value: &str, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let value = CString::new(value)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "trace value contains NUL"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptracecontextset(value.as_ptr(), flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptracecontextset(self.c_ctx_ptr(), value.as_ptr(), flags) };
        self.rc_to_result(rc)
    }

    /// Set `value` as W3C tracestate on the active span; empty clears it.
    /// `flags` is reserved and must be zero. Native code copies `value`.
    pub fn tptracestateset(&self, value: &str, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let value = CString::new(value)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "trace value contains NUL"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptracestateset(value.as_ptr(), flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptracestateset(self.c_ctx_ptr(), value.as_ptr(), flags) };
        self.rc_to_result(rc)
    }

    /// Return the current W3C traceparent, or an empty string when absent.
    /// `flags` is reserved and must be zero.
    pub fn tptracecontextget(&self, flags: i64) -> AtmiResult<String> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let mut value = [0 as c_char; raw::NDRX_TRACEPARENT_LEN as usize + 1];
        let mut length = value.len() as c_long;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptracecontextget(value.as_mut_ptr(), &mut length, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otptracecontextget(self.c_ctx_ptr(), value.as_mut_ptr(), &mut length, flags)
        };
        self.rc_to_result(rc)?;
        Ok(crate::fixed_cstr_to_string(&value))
    }

    /// Return the current W3C tracestate, or an empty string when absent.
    /// `flags` is reserved and must be zero.
    pub fn tptracestateget(&self, flags: i64) -> AtmiResult<String> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let mut value = [0 as c_char; raw::NDRX_TRACE_STATE_MAX as usize + 1];
        let mut length = value.len() as c_long;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptracestateget(value.as_mut_ptr(), &mut length, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otptracestateget(self.c_ctx_ptr(), value.as_mut_ptr(), &mut length, flags)
        };
        self.rc_to_result(rc)?;
        Ok(crate::fixed_cstr_to_string(&value))
    }

    /// Start a named span with optional UBF attributes and return its handle.
    /// `flags` selects `TPTRACE_F_ACTIVE` and/or `TPTRACE_F_NEW_ROOT`.
    /// Active spans must end in reverse nesting order, in this same context.
    pub fn tptracespanbegin(
        &self,
        name: &str,
        attributes: Option<&TypedUbf<'_>>,
        flags: i64,
    ) -> AtmiResult<TpTraceSpan> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let name = CString::new(name)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "span name contains NUL"))?;
        let attributes = attributes.map_or(std::ptr::null(), |a| a.as_ubfh());
        let mut handle = 0;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptracespanbegin(name.as_ptr(), attributes, flags, &mut handle) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otptracespanbegin(
                self.c_ctx_ptr(),
                name.as_ptr(),
                attributes,
                flags,
                &mut handle,
            )
        };
        self.rc_to_result(rc)?;
        Ok(TpTraceSpan(handle))
    }

    /// End `span` with a `TPTRACE_STATUS_*` status and optional UBF attributes.
    /// `flags` must be zero. A successfully ended span cannot be reused.
    pub fn tptracespanend(
        &self,
        span: TpTraceSpan,
        status: i32,
        attributes: Option<&TypedUbf<'_>>,
        flags: i64,
    ) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let attributes = attributes.map_or(std::ptr::null(), |a| a.as_ubfh());
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptracespanend(span.0 as _, status, attributes, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otptracespanend(self.c_ctx_ptr(), span.0 as _, status, attributes, flags)
        };
        self.rc_to_result(rc)
    }

    /// Record `name` and optional UBF attributes on the current span; `flags` must be zero.
    pub fn tptraceevent(
        &self,
        name: &str,
        attributes: Option<&TypedUbf<'_>>,
        flags: i64,
    ) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let name = CString::new(name)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "event name contains NUL"))?;
        let attributes = attributes.map_or(std::ptr::null(), |a| a.as_ubfh());
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptraceevent(name.as_ptr(), attributes, flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptraceevent(self.c_ctx_ptr(), name.as_ptr(), attributes, flags) };
        self.rc_to_result(rc)
    }

    /// Copy `key` and `value` into the scope and destinations selected by `TPCTXTAG_*` flags.
    /// NEXT tags persist until explicitly cleared; they are not consumed after one record.
    pub fn tpcontexttagset(&self, key: &str, value: &str, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let key =
            CString::new(key).map_err(|_| AtmiError::new(raw::TPEINVAL, "tag key contains NUL"))?;
        let value = CString::new(value)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "tag value contains NUL"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpcontexttagset(key.as_ptr(), value.as_ptr(), flags) };
        #[cfg(feature = "ctx-send")]
        let rc =
            unsafe { raw::Otpcontexttagset(self.c_ctx_ptr(), key.as_ptr(), value.as_ptr(), flags) };
        self.rc_to_result(rc)
    }

    /// Remove `key` from the scopes and destinations selected by `TPCTXTAG_*` flags.
    pub fn tpcontexttagclear(&self, key: &str, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        let key =
            CString::new(key).map_err(|_| AtmiError::new(raw::TPEINVAL, "tag key contains NUL"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpcontexttagclear(key.as_ptr(), flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpcontexttagclear(self.c_ctx_ptr(), key.as_ptr(), flags) };
        self.rc_to_result(rc)
    }

    /// Clear every tag in the scopes and destinations selected by `TPCTXTAG_*` flags.
    pub fn tpcontexttagclearall(&self, flags: i64) -> AtmiResult<()> {
        let flags = c_long::try_from(flags)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "flags exceed native long"))?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpcontexttagclearall(flags) };
        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpcontexttagclearall(self.c_ctx_ptr(), flags) };
        self.rc_to_result(rc)
    }
}
