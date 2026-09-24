//! Subsystem-specific errors that preserve native Enduro/X codes and diagnostic messages.
use crate::raw;
use std::{borrow::Cow, error::Error, fmt};

// --- ATMI Errors -------------------------------------------------------------

/// Export each native code once and retain the associated error-type spelling.
macro_rules! gen_error_consts {
    ($error:ident { $($name:ident),* $(,)? }) => {
        $(
            #[doc = concat!("Native `", stringify!($name), "` error code for [`", stringify!($error), "`].")]
            pub const $name: u32 = raw::$name;
        )*
        impl $error {
            $(
                #[doc = concat!("Native error code; see [`", stringify!($name), "`](crate::", stringify!($name), ").")]
                pub const $name: u32 = crate::errors::$name;
            )*
        }
    };
}

/// An XATMI error with its native code and diagnostic text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtmiError {
    pub code: u32,
    pub message: Cow<'static, str>,
}

/// ATMI error construction and native error-code constants.
impl AtmiError {
    /// Create an error preserving the native error code and diagnostic message.
    ///
    /// # Arguments
    ///
    /// - `code`: Native error number for this error subsystem.
    /// - `message`: Diagnostic text, either a static string or an owned string.
    pub fn new(code: u32, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

gen_error_consts! {
    AtmiError {
        TPMINVAL,
        TPEABORT,
        TPEBADDESC,
        TPEBLOCK,
        TPEINVAL,
        TPELIMIT,
        TPENOENT,
        TPEOS,
        TPEPERM,
        TPEPROTO,
        TPESVCERR,
        TPESVCFAIL,
        TPESYSTEM,
        TPETIME,
        TPETRAN,
        TPGOTSIG,
        TPERMERR,
        TPEITYPE,
        TPEOTYPE,
        TPERELEASE,
        TPEHAZARD,
        TPEHEURISTIC,
        TPEEVENT,
        TPEMATCH,
        TPEDIAGNOSTIC,
        TPEMIB,
        TPERFU26,
        TPERFU27,
        TPERFU28,
        TPERFU29,
        TPINITFAIL,
        TPMAXVAL,
    }
}

/// Human-readable formatting that includes native error codes and diagnostics.
impl fmt::Display for AtmiError {
    /// Write the error code and message to the display formatter.
    ///
    /// # Arguments
    ///
    /// - `f`: Formatter receiving the human-readable error.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[code {}] {}", self.code, self.message)
    }
}

/// Integration with Rust’s standard error trait.
impl Error for AtmiError {}

/// Result of an XATMI operation.
pub type AtmiResult<T> = Result<T, AtmiError>;

// --- UBF Errors --------------------------------------------------------------

/// A UBF or VIEW error with its native code and diagnostic text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UbfError {
    pub code: u32,
    pub message: Cow<'static, str>,
}

/// UBF error construction and native error-code constants.
impl UbfError {
    /// Create an error preserving the native error code and diagnostic message.
    ///
    /// # Arguments
    ///
    /// - `code`: Native error number for this error subsystem.
    /// - `message`: Diagnostic text, either a static string or an owned string.
    pub fn new(code: u32, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

gen_error_consts! {
    UbfError {
        BMINVAL,
        BERFU0,
        BALIGNERR,
        BNOTFLD,
        BNOSPACE,
        BNOTPRES,
        BBADFLD,
        BTYPERR,
        BEUNIX,
        BBADNAME,
        BMALLOC,
        BSYNTAX,
        BFTOPEN,
        BFTSYNTAX,
        BEINVAL,
        BERFU1,
        BBADTBL,
        BBADVIEW,
        BVFSYNTAX,
        BVFOPEN,
        BBADACM,
        BNOCNAME,
        BEBADOP,
        BMAXVAL,
    }
}

/// Human-readable formatting that includes native error codes and diagnostics.
impl fmt::Display for UbfError {
    /// Write the error code and message to the display formatter.
    ///
    /// # Arguments
    ///
    /// - `f`: Formatter receiving the human-readable error.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[code {}] {}", self.code, self.message)
    }
}

/// Integration with Rust’s standard error trait.
impl Error for UbfError {}

/// Result of a UBF or VIEW operation.
pub type UbfResult<T> = Result<T, UbfError>;

// --- NSTD Errors -------------------------------------------------------------

/// An Enduro/X standard-utility error with its native code and diagnostic text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NstdError {
    pub code: u32,
    pub message: Cow<'static, str>,
}

/// NSTD error construction and native error-code constants.
impl NstdError {
    /// Create an error preserving the native error code and diagnostic message.
    ///
    /// # Arguments
    ///
    /// - `code`: Native error number for this error subsystem.
    /// - `message`: Diagnostic text, either a static string or an owned string.
    pub fn new(code: u32, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

gen_error_consts! {
    NstdError {
        NMINVAL,
        NEINVALINI,
        NEMALLOC,
        NEUNIX,
        NEINVAL,
        NESYSTEM,
        NEMANDATORY,
        NEFORMAT,
        NETOUT,
        NENOCONN,
        NELIMIT,
        NEPLUGIN,
        NENOSPACE,
        NEINVALKEY,
        NENOENT,
        NEWRITE,
        NEEXEC,
        NESUPPORT,
        NEEXISTS,
        NEVERSION,
        NEBUSY,
        NESTALE,
        NEEOF,
        NESYNC,
        NECONSTRAINT,
        NESTATE,
        NEUNKNOWN,
        NESTMT,
        NEACCESS,
        NECONTEXT,
        NEMATCH,
        NEPROTO,
        NEUBF,
        NEATMI,
        NEMSGSIZE,
        NECANCELED,
        NEINPROGRESS,
        NEDEADLK,
        NEPRECOND,
        NEUNAVAILABLE,
        NMAXVAL,
    }
}

/// Human-readable formatting that includes native error codes and diagnostics.
impl fmt::Display for NstdError {
    /// Write the error code and message to the display formatter.
    ///
    /// # Arguments
    ///
    /// - `f`: Formatter receiving the human-readable error.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[code {}] {}", self.code, self.message)
    }
}

/// Integration with Rust’s standard error trait.
impl Error for NstdError {}

/// Result of an Enduro/X standard-utility operation.
pub type NstdResult<T> = Result<T, NstdError>;
