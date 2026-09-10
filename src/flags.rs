use crate::raw;

pub const TPNOBLOCK: i64 = raw::TPNOBLOCK as i64;
pub const TPSIGRSTRT: i64 = raw::TPSIGRSTRT as i64;
pub const TPNOREPLY: i64 = raw::TPNOREPLY as i64;
pub const TPNOTRAN: i64 = raw::TPNOTRAN as i64;
pub const TPTRAN: i64 = raw::TPTRAN as i64;
pub const TPNOTIME: i64 = raw::TPNOTIME as i64;
pub const TPGETANY: i64 = raw::TPGETANY as i64;
pub const TPNOCHANGE: i64 = raw::TPNOCHANGE as i64;
pub const TPCONV: i64 = raw::TPCONV as i64;
pub const TPSENDONLY: i64 = raw::TPSENDONLY as i64;
pub const TPRECVONLY: i64 = raw::TPRECVONLY as i64;
pub const TPTRANSUSPEND: i64 = raw::TPTRANSUSPEND as i64;

/// `tpsblktime`/`tpgblktime`: apply the timeout to the next call only.
pub const TPBLK_NEXT: i64 = raw::TPBLK_NEXT as i64;
/// `tpsblktime`/`tpgblktime`: apply the timeout to every call on this thread.
pub const TPBLK_ALL: i64 = raw::TPBLK_ALL as i64;

/// `tpencrypt`/`tpdecrypt`: input and output are NUL-terminated base64 strings.
pub const TPEX_STRING: i64 = raw::TPEX_STRING as i64;
/// Interpret a message priority as an absolute value in the range 1..=100.
pub const TPABSOLUTE: i64 = raw::TPABSOLUTE as i64;
