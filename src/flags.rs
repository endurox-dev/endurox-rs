//! Native XATMI option bits. Combine supported flags with `|`; pass `0` for defaults.
use crate::raw;

/// Fail immediately when a send queue is full or a requested receive has no data.
pub const TPNOBLOCK: i64 = raw::TPNOBLOCK as i64;
/// Restart an interrupted native blocking operation after a signal.
pub const TPSIGRSTRT: i64 = raw::TPSIGRSTRT as i64;
/// Submit without requesting a service reply; invalid for `tpcall`.
pub const TPNOREPLY: i64 = raw::TPNOREPLY as i64;
/// Exclude this operation from the caller’s global transaction.
pub const TPNOTRAN: i64 = raw::TPNOTRAN as i64;
/// Indicate that a service request belongs to a global transaction.
pub const TPTRAN: i64 = raw::TPTRAN as i64;
/// Disable the normal XATMI blocking timeout for this operation.
pub const TPNOTIME: i64 = raw::TPNOTIME as i64;
/// Receive any eligible outstanding reply and report its call descriptor.
pub const TPGETANY: i64 = raw::TPGETANY as i64;
/// Reject a reply whose buffer type or subtype differs from the destination.
pub const TPNOCHANGE: i64 = raw::TPNOCHANGE as i64;
/// Indicate a conversational service request.
pub const TPCONV: i64 = raw::TPCONV as i64;
/// Give the caller initial send control when opening a conversation.
pub const TPSENDONLY: i64 = raw::TPSENDONLY as i64;
/// Open in receive mode, or hand conversation send control to the peer.
pub const TPRECVONLY: i64 = raw::TPRECVONLY as i64;
/// Suspend the caller’s global transaction around the native call; unsupported by async reply
/// routing.
pub const TPTRANSUSPEND: i64 = raw::TPTRANSUSPEND as i64;

/// `tpsblktime`/`tpgblktime`: apply the timeout to the next call only.
pub const TPBLK_NEXT: i64 = raw::TPBLK_NEXT as i64;
/// `tpsblktime`/`tpgblktime`: apply the timeout to every call on this thread.
pub const TPBLK_ALL: i64 = raw::TPBLK_ALL as i64;

/// Select string/base64 handling for import/export and native encryption APIs.
/// Use the string encryption methods; the byte-slice encryption methods reject this flag.
pub const TPEX_STRING: i64 = raw::TPEX_STRING as i64;
/// Interpret a message priority as an absolute value in the range 1..=100.
pub const TPABSOLUTE: i64 = raw::TPABSOLUTE as i64;
