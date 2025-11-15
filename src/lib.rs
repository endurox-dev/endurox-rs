#![allow(dead_code, non_camel_case_types, non_snake_case, non_upper_case_globals)]
pub(crate) mod raw { include!(concat!(env!("OUT_DIR"), "/bindings.rs")); }

// your high-level modules
mod atmictx;
mod atmictx_log;
mod errors;
mod typed_buf;
mod typed_ubf;

// re-export the public façade so external users/tests can `use endurox_rs::AtmiCtx`
pub use errors::{AtmiError, AtmiResult};
pub use atmictx::AtmiCtx;
pub use atmictx_log::LogLevel;
pub use typed_buf::TypedBuffer;
pub use typed_ubf::TypedUbf;
