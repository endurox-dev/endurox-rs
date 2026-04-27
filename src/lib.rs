#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
pub(crate) mod raw {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[doc(hidden)]
pub mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/test.rs"));
}

mod atmictx;
mod atmictx_log;
mod atmictx_srv;
mod atmictx_ubf;
mod atmictx_xatmi;
mod errors;
mod flags;
mod tpsvcinfo;
mod typed_buf;
mod typed_ubf;
mod types;

pub use atmictx::AtmiCtx;
pub use atmictx_log::LogLevel;
pub use atmictx_srv::{
    PollerEvent, RustBeforePollCallback, RustPeriodCallback, RustPollerCallback,
    RustServerDoneHook, RustServerInitHook, RustServiceCallback, TpReturnStatus,
};
pub use atmictx_ubf::UbfFieldType;
pub use errors::{AtmiError, AtmiResult, NstdError, NstdResult, UbfError, UbfResult};
pub use flags::{
    TPCONV, TPGETANY, TPNOBLOCK, TPNOCHANGE, TPNOREPLY, TPNOTIME, TPNOTRAN, TPRECVONLY, TPSENDONLY,
    TPSIGRSTRT, TPTRAN, TPTRANSUSPEND,
};
pub use tpsvcinfo::TpSvcInfo;
pub use typed_buf::TypedBuffer;
pub use typed_ubf::{BorrowedUbf, TypedUbf, UbfValue};
pub use types::{ClientId, TpContext, TpTranId};

/// Event subscription control block used by [`AtmiCtx::tpsubscribe`].
///
/// This is an opaque Rust wrapper around the Enduro/X `TPEVCTL` structure.
/// Use [`Default::default`] when no fields need to be customized.
pub struct TpEvCtl {
    inner: raw::TPEVCTL,
}

impl Default for TpEvCtl {
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

impl TpEvCtl {
    #[inline]
    pub(crate) fn as_mut_ptr(&mut self) -> *mut raw::TPEVCTL {
        &mut self.inner
    }
}

/// Persistent queue control block used by queue enqueue/dequeue APIs.
///
/// This is an opaque Rust wrapper around the Enduro/X `TPQCTL` structure.
/// Use [`Default::default`] when no fields need to be customized.
pub struct TpQCtl {
    inner: raw::TPQCTL,
}

impl Default for TpQCtl {
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

impl TpQCtl {
    #[inline]
    pub(crate) fn as_mut_ptr(&mut self) -> *mut raw::TPQCTL {
        &mut self.inner
    }
}
