//! Subsystem-specific errors that preserve native Enduro/X codes and diagnostic messages.
use crate::raw;
use std::{borrow::Cow, error::Error, fmt};

// --- ATMI Errors -------------------------------------------------------------

/// Expose native error-number constants under the enclosing error type.
macro_rules! gen_error_consts {
    ($($name:ident),* $(,)?) => {
        $(pub const $name: u32 = raw::$name;)*
    };
}

/// An XATMI error with its native code and diagnostic text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtmiError {
    pub code: u32,
    pub message: Cow<'static, str>,
}

/* ATMI error */
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

    // List of errors codes
    gen_error_consts! {
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
        TPMAXVAL
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

/* ATMI error */
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

    // List of errors codes
    gen_error_consts! {
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
        BMAXVAL
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

/* ATMI error */
/// NSTD error construction preserving native diagnostics.
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
