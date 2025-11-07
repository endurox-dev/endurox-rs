#![allow(dead_code, non_camel_case_types, non_snake_case, non_upper_case_globals)]
pub(crate) mod raw { include!(concat!(env!("OUT_DIR"), "/bindings.rs")); }

// your high-level modules
mod atmictx;

// re-export the public façade so external users/tests can `use endurox_rs::AtmiCtx`
pub use atmictx::AtmiCtx;
