/// Opaque ATMI client identifier returned by `TpSvcInfo::cltid()`.
/// Pass it to `AtmiCtx::tpnotify()`.
pub type ClientId = crate::raw::CLIENTID;

/// Transaction identifier filled by `tpsuspend` and consumed by `tpresume`.
#[derive(Debug)]
pub struct TpTranId(pub(crate) crate::raw::TPTRANID);

impl TpTranId {
    /// # Safety
    /// Must be a valid TPTRANID previously populated by `tpsuspend`.
    pub(crate) unsafe fn from_raw(inner: crate::raw::TPTRANID) -> Self {
        TpTranId(inner)
    }
}
