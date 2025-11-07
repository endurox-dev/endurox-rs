use std::{borrow::Cow, error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtmiError {
    pub code: u16,
    pub message: Cow<'static, str>,
}

/* ATMI error */
impl AtmiError {
    pub fn new(code: u16, message: impl Into<Cow<'static, str>>) -> Self {
        Self { code, message: message.into() }
    }

    /*
    // Optional convenience constructors:
    pub fn bad_request(msg: impl Into<Cow<'static, str>>) -> Self {
        Self::new(400, msg)
    }
    pub fn not_found(msg: impl Into<Cow<'static, str>>) -> Self {
        Self::new(404, msg)
    }
    pub fn internal(msg: impl Into<Cow<'static, str>>) -> Self {
        Self::new(500, msg)
    }
    */
}

impl fmt::Display for AtmiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[code {}] {}", self.code, self.message)
    }
}

impl Error for AtmiError {}

pub type AtmiResult<T> = Result<T, AtmiError>;

