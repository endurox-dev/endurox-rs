#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
pub(crate) mod raw {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

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

// Re-export complex C structs users may need to fill when calling advanced APIs
// (tpsubscribe, tpenqueue/tpdequeue).  Named without the raw:: prefix so callers
// never need to reference the internal `raw` module.
pub use raw::{TPEVCTL as TpEvCtl, TPQCTL as TpQCtl};
