//! Native client and transaction identifiers used by the safe XATMI wrappers.
/// Opaque ATMI client identifier returned by `TpSvcInfo::cltid()`.
/// Pass it to `AtmiCtx::tpnotify()`.
pub type ClientId = crate::raw::CLIENTID;

/// Transaction identifier filled by `tpsuspend` and consumed by `tpresume`.
#[derive(Debug)]
pub struct TpTranId(pub(crate) crate::raw::TPTRANID);

/// Wrapping of native suspended-transaction identifiers.
impl TpTranId {
    /// Wrap a native suspended-transaction identifier.
    ///
    /// # Arguments
    ///
    /// - `inner`: Transaction identifier populated by a successful native `tpsuspend` call.
    ///
    /// # Safety
    /// Must be a valid TPTRANID previously populated by `tpsuspend`.
    pub(crate) unsafe fn from_raw(inner: crate::raw::TPTRANID) -> Self {
        TpTranId(inner)
    }
}
