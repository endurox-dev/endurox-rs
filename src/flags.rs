//! Native XATMI and UBF constants from the selected Enduro/X headers.
//!
//! Operation flags use `i64`, field types and VIEW conversion modes use `i32`,
//! and buffer limits use `usize`. Queue diagnostics and context sentinels stay signed.
//! Reserved native values are exported for completeness; check each method's contract.
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

// Persistent queue control and quality-of-service flags.
/// Set/get correlation id.
pub const TPQCORRID: i64 = raw::TPQCORRID as i64;
/// Set/get failure queue.
pub const TPQFAILUREQ: i64 = raw::TPQFAILUREQ as i64;
/// Reserved for future use: enqueue before message id.
pub const TPQBEFOREMSGID: i64 = raw::TPQBEFOREMSGID as i64;
/// Reserved for future use: deprecated.
pub const TPQGETBYMSGIDOLD: i64 = raw::TPQGETBYMSGIDOLD as i64;
/// Get msgid of enq/deq message.
pub const TPQMSGID: i64 = raw::TPQMSGID as i64;
/// Set/get message priority.
pub const TPQPRIORITY: i64 = raw::TPQPRIORITY as i64;
/// Reserved for future use: enqueue at queue top.
pub const TPQTOP: i64 = raw::TPQTOP as i64;
/// Reserved for future use: wait for dequeuing.
pub const TPQWAIT: i64 = raw::TPQWAIT as i64;
/// Set/get reply queue.
pub const TPQREPLYQ: i64 = raw::TPQREPLYQ as i64;
/// Set absolute time.
pub const TPQTIME_ABS: i64 = raw::TPQTIME_ABS as i64;
/// Set relative time.
pub const TPQTIME_REL: i64 = raw::TPQTIME_REL as i64;
/// Deprecated.
pub const TPQGETBYCORRIDOLD: i64 = raw::TPQGETBYCORRIDOLD as i64;
/// Peek.
pub const TPQPEEK: i64 = raw::TPQPEEK as i64;
/// Reserved for future use: delivery quality of service.
pub const TPQDELIVERYQOS: i64 = raw::TPQDELIVERYQOS as i64;
/// Reserved for future use: reply message quality of service.
pub const TPQREPLYQOS: i64 = raw::TPQREPLYQOS as i64;
/// Reserved for future use: absolute expiration time.
pub const TPQEXPTIME_ABS: i64 = raw::TPQEXPTIME_ABS as i64;
/// Reserved for future use: relative expiration time.
pub const TPQEXPTIME_REL: i64 = raw::TPQEXPTIME_REL as i64;
/// Reserved for future use: never expire.
pub const TPQEXPTIME_NONE: i64 = raw::TPQEXPTIME_NONE as i64;
/// Dequeue by msgid.
pub const TPQGETBYMSGID: i64 = raw::TPQGETBYMSGID as i64;
/// Dequeue by corrid.
pub const TPQGETBYCORRID: i64 = raw::TPQGETBYCORRID as i64;
/// Async complete.
pub const TPQASYNC: i64 = raw::TPQASYNC as i64;
/// Keep originator data.
pub const TPQKEEPORIG: i64 = raw::TPQKEEPORIG as i64;
/// Queue's default persistence policy.
pub const TPQQOSDEFAULTPERSIST: i64 = raw::TPQQOSDEFAULTPERSIST as i64;
/// Disk message.
pub const TPQQOSPERSISTENT: i64 = raw::TPQQOSPERSISTENT as i64;
/// Memory message.
pub const TPQQOSNONPERSISTENT: i64 = raw::TPQQOSNONPERSISTENT as i64;

/// Deliver matching events to the service in [`crate::TpEvCtl::name1`].
pub const TPEVSERVICE: i64 = raw::TPEVSERVICE as i64;

/// Keep a subscription when its destination service becomes unavailable.
pub const TPEVPERSIST: i64 = raw::TPEVPERSIST as i64;

// Remaining XATMI operation, context, conversion and transaction constants.
/// Native acknowledgment option.
pub const TPACK: i64 = raw::TPACK as i64;
/// Software-raised service error flag.
pub const TPSOFTERR: i64 = raw::TPSOFTERR as i64;
/// Software-raised timeout condition.
pub const TPSOFTTIMEOUT: i64 = raw::TPSOFTTIMEOUT as i64;
/// Software-raised service-not-found condition.
pub const TPSOFTENOENT: i64 = raw::TPSOFTENOENT as i64;
/// Do not restore the automatic buffer in a server context.
pub const TPNOAUTBUF: i64 = raw::TPNOAUTBUF as i64;
/// Use regular-expression matching where supported.
pub const TPREGEXMATCH: i64 = raw::TPREGEXMATCH as i64;
/// Skip cache lookup.
pub const TPNOCACHELOOK: i64 = raw::TPNOCACHELOOK as i64;
/// Do not add the result to the cache.
pub const TPNOCACHEADD: i64 = raw::TPNOCACHEADD as i64;
/// Do not use cached data.
pub const TPNOCACHEDDATA: i64 = raw::TPNOCACHEDDATA as i64;
/// Do not abort the transaction on service failure; unsupported by async reply routing.
pub const TPNOABORT: i64 = raw::TPNOABORT as i64;
/// Reserved for future use.
pub const TPEVQUEUE: i64 = raw::TPEVQUEUE as i64;
/// Reserved for future use.
pub const TPEVTRAN: i64 = raw::TPEVTRAN as i64;
/// Reject an import that would change the destination buffer type.
pub const TPEX_NOCHANGE: i64 = raw::TPEX_NOCHANGE as i64;
/// Import min version.
pub const TPIMPEXP_VERSION_MIN: u32 = raw::TPIMPEXP_VERSION_MIN;
/// Import / export max version.
pub const TPIMPEXP_VERSION_MAX: u32 = raw::TPIMPEXP_VERSION_MAX;
/// Native service failure status; see [`crate::TpReturnStatus::Fail`].
pub const TPFAIL: i32 = raw::TPFAIL as i32;
/// Native service success status; see [`crate::TpReturnStatus::Success`].
pub const TPSUCCESS: i32 = raw::TPSUCCESS as i32;
/// Native service failure status requesting server shutdown.
pub const TPEXIT: i32 = raw::TPEXIT as i32;
/// Conversation disconnected immediately.
pub const TPEV_DISCONIMM: i64 = raw::TPEV_DISCONIMM as i64;
/// Conversation ended with a service error.
pub const TPEV_SVCERR: i64 = raw::TPEV_SVCERR as i64;
/// Conversation ended with service failure.
pub const TPEV_SVCFAIL: i64 = raw::TPEV_SVCFAIL as i64;
/// Conversation ended with service success.
pub const TPEV_SVCSUCC: i64 = raw::TPEV_SVCSUCC as i64;
/// Conversation peer transferred send control.
pub const TPEV_SENDONLY: i64 = raw::TPEV_SENDONLY as i64;
/// Mask for unsolicited-message notification modes.
pub const TPU_MASK: i64 = raw::TPU_MASK as i64;
/// Reserved for future use.
pub const TPU_SIG: i64 = raw::TPU_SIG as i64;
/// Dispatch unsolicited messages when entering supported XATMI calls.
pub const TPU_DIP: i64 = raw::TPU_DIP as i64;
/// Ignore unsol messages.
pub const TPU_IGN: i64 = raw::TPU_IGN as i64;
/// Native fast-path initialization option for Tuxedo compatibility.
pub const TPSA_FASTPATH: i64 = raw::TPSA_FASTPATH as i64;
/// Native protected initialization option for Tuxedo compatibility.
pub const TPSA_PROTECTED: i64 = raw::TPSA_PROTECTED as i64;
/// Allow `tpgetcallinfo` to report missing call information without an error.
pub const TPCI_NOEOFERR: i64 = raw::TPCI_NOEOFERR as i64;
/// Do not generate error if group is not singleton.
pub const TPPG_NONSGSUCC: i64 = raw::TPPG_NONSGSUCC as i64;
/// Do node verification, if configured.
pub const TPPG_SGVERIFY: i64 = raw::TPPG_SGVERIFY as i64;
/// Set if group is singleton.
pub const TPPG_SINGLETON: i64 = raw::TPPG_SINGLETON as i64;
/// Max identifier buffer.
pub const TPCONVMAXSTR: usize = raw::TPCONVMAXSTR as usize;
/// Convert to string.
pub const TPTOSTRING: i64 = raw::TPTOSTRING as i64;
/// Convert client id.
pub const TPCONVCLTID: i64 = raw::TPCONVCLTID as i64;
/// Convert transaction id.
pub const TPCONVTRANID: i64 = raw::TPCONVTRANID as i64;
/// Select native XID conversion; currently unsupported by Enduro/X.
pub const TPCONVXID: i64 = raw::TPCONVXID as i64;
/// Invalid native context sentinel; not an [`crate::AtmiCtx`] handle.
pub const TPINVALIDCONTEXT: isize = raw::TPINVALIDCONTEXT as isize;
/// Legacy single-context sentinel; unused by Enduro/X.
pub const TPSINGLECONTEXT: isize = raw::TPSINGLECONTEXT as isize;
/// Null native context sentinel.
pub const TPNULLCONTEXT: isize = raw::TPNULLCONTEXT as isize;
/// Enable native multiple-context initialization.
pub const TPMULTICONTEXTS: i64 = raw::TPMULTICONTEXTS as i64;
/// No native operation flags.
pub const TPNOFLAGS: i64 = raw::TPNOFLAGS as i64;
/// Reserved for future use: no authentication.
pub const TPNOAUTH: i32 = raw::TPNOAUTH as i32;
/// Reserved for future use: system authentication.
pub const TPSYSAUTH: i32 = raw::TPSYSAUTH as i32;
/// Reserved for future use: system and application authentication.
pub const TPAPPAUTH: i32 = raw::TPAPPAUTH as i32;
/// Q identifier is client.
pub const TPMYIDTYP_CLIENT: i32 = raw::TPMYIDTYP_CLIENT as i32;
/// Q identifier is server.
pub const TPMYIDTYP_SERVER: i32 = raw::TPMYIDTYP_SERVER as i32;
/// Commit decision logged.
pub const TPTXCOMMITDLOG: i64 = raw::TPTXCOMMITDLOG as i64;
/// No known host optimization.
pub const TPTXNOOPTIM: i64 = raw::TPTXNOOPTIM as i64;
/// Use TMSUSPEND (keep assoc).
pub const TPTXTMSUSPEND: i64 = raw::TPTXTMSUSPEND as i64;
/// Open all RMs.
pub const TPTXOPENALL: i64 = raw::TPTXOPENALL as i64;
/// Return after commit has logged.
pub const TP_CMT_LOGGED: i64 = raw::TP_CMT_LOGGED as i64;
/// Return after commit has completed.
pub const TP_CMT_COMPLETE: i64 = raw::TP_CMT_COMPLETE as i64;
/// Report previous nonping reconnect.
pub const TPEXT_RMPING_PREVRECON: i64 = raw::TPEXT_RMPING_PREVRECON as i64;
/// Xa_recover() based ping.
pub const TPEXT_RMPING_MODE2: i64 = raw::TPEXT_RMPING_MODE2 as i64;
/// Valid flags.
pub const TPBLK__MASK: i64 = raw::TPBLK__MASK as i64;

// Logging-query flags and encoded-level shift.
/// Native `TPLOGQI_RET_HAVDETAILED` constant.
pub const TPLOGQI_RET_HAVDETAILED: i64 = raw::TPLOGQI_RET_HAVDETAILED as i64;
/// Native `TPLOGQI_RET_DBGLEVBITS` constant.
pub const TPLOGQI_RET_DBGLEVBITS: u32 = raw::TPLOGQI_RET_DBGLEVBITS;
/// Native `TPLOGQI_GET_NDRX` constant.
pub const TPLOGQI_GET_NDRX: i64 = raw::TPLOGQI_GET_NDRX as i64;
/// Native `TPLOGQI_GET_UBF` constant.
pub const TPLOGQI_GET_UBF: i64 = raw::TPLOGQI_GET_UBF as i64;
/// Native `TPLOGQI_GET_TP` constant.
pub const TPLOGQI_GET_TP: i64 = raw::TPLOGQI_GET_TP as i64;
/// Native `TPLOGQI_EVAL_DETAILED` constant.
pub const TPLOGQI_EVAL_DETAILED: i64 = raw::TPLOGQI_EVAL_DETAILED as i64;
/// Native `TPLOGQI_EVAL_RETURN` constant.
pub const TPLOGQI_EVAL_RETURN: i64 = raw::TPLOGQI_EVAL_RETURN as i64;

// Persistent-queue diagnostics returned by TpQCtl::diagnostic().
/// Invalid persistent-queue operation.
pub const QMEINVAL: i64 = raw::QMEINVAL as i64;
/// Invalid queue resource-manager identifier.
pub const QMEBADRMID: i64 = raw::QMEBADRMID as i64;
/// Queue resource manager is not open.
pub const QMENOTOPEN: i64 = raw::QMENOTOPEN as i64;
/// Queue transaction error.
pub const QMETRAN: i64 = raw::QMETRAN as i64;
/// Invalid queue message identifier.
pub const QMEBADMSGID: i64 = raw::QMEBADMSGID as i64;
/// Queue subsystem failure.
pub const QMESYSTEM: i64 = raw::QMESYSTEM as i64;
/// Queue operating-system error.
pub const QMEOS: i64 = raw::QMEOS as i64;
/// Queue transaction was aborted.
pub const QMEABORTED: i64 = raw::QMEABORTED as i64;
/// Alias for [`QMEABORTED`].
pub const QMENOTA: i64 = raw::QMENOTA as i64;
/// Queue protocol error.
pub const QMEPROTO: i64 = raw::QMEPROTO as i64;
/// Invalid queue.
pub const QMEBADQUEUE: i64 = raw::QMEBADQUEUE as i64;
/// No matching queue message.
pub const QMENOMSG: i64 = raw::QMENOMSG as i64;
/// Queue resource is in use.
pub const QMEINUSE: i64 = raw::QMEINUSE as i64;
/// No space for the queue operation.
pub const QMENOSPACE: i64 = raw::QMENOSPACE as i64;
/// Queue release mismatch.
pub const QMERELEASE: i64 = raw::QMERELEASE as i64;
/// Invalid queue handle.
pub const QMEINVHANDLE: i64 = raw::QMEINVHANDLE as i64;
/// Queue sharing error.
pub const QMESHARE: i64 = raw::QMESHARE as i64;

// Public XATMI identifier and queue-control sizes.
/// Maximum native identifier length.
pub const MAXTIDENT: usize = raw::MAXTIDENT as usize;
/// Native `XATMI_SERVICE_NAME_LENGTH` constant.
pub const XATMI_SERVICE_NAME_LENGTH: usize = raw::XATMI_SERVICE_NAME_LENGTH as usize;
/// Max type len.
pub const XATMI_TYPE_LEN: usize = raw::XATMI_TYPE_LEN as usize;
/// Max sub-type len.
pub const XATMI_SUBTYPE_LEN: usize = raw::XATMI_SUBTYPE_LEN as usize;
/// Max len of event to bcast.
pub const XATMI_EVENT_MAX: usize = raw::XATMI_EVENT_MAX as usize;
/// Native `TMQNAMELEN` constant.
pub const TMQNAMELEN: usize = raw::TMQNAMELEN as usize;
/// Native `TMMSGIDLEN` constant.
pub const TMMSGIDLEN: usize = raw::TMMSGIDLEN as usize;
/// TMMSGIDLEN * 1.4 (base64 overhead).
pub const TMMSGIDLEN_STR: usize = raw::TMMSGIDLEN_STR as usize;
/// Native `TMCORRIDLEN` constant.
pub const TMCORRIDLEN: usize = raw::TMCORRIDLEN as usize;
/// TMCORRIDLEN * 1.4 (base64 overhead).
pub const TMCORRIDLEN_STR: usize = raw::TMCORRIDLEN_STR as usize;

// UBF field types, iteration sentinels, limits and VIEW conversion modes.
/// Native UBF buffer format version.
pub const UBF_VERSION: u32 = raw::UBF_VERSION;
/// Maximum UBFH length.
pub const MAXUBFLEN: usize = raw::MAXUBFLEN as usize;
/// Max UBF buffer field len.
pub const UBFFLDMAX: usize = raw::UBFFLDMAX as usize;
/// Minimum native field type code.
pub const BFLD_MIN: i32 = raw::BFLD_MIN as i32;
/// Native `BFLD_SHORT` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_SHORT: i32 = raw::BFLD_SHORT as i32;
/// Native `BFLD_LONG` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_LONG: i32 = raw::BFLD_LONG as i32;
/// Native `BFLD_CHAR` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_CHAR: i32 = raw::BFLD_CHAR as i32;
/// Native `BFLD_FLOAT` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_FLOAT: i32 = raw::BFLD_FLOAT as i32;
/// Native `BFLD_DOUBLE` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_DOUBLE: i32 = raw::BFLD_DOUBLE as i32;
/// Native `BFLD_STRING` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_STRING: i32 = raw::BFLD_STRING as i32;
/// Native `BFLD_CARRAY` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_CARRAY: i32 = raw::BFLD_CARRAY as i32;
/// Native VIEW integer type; not a UBF field kind.
pub const BFLD_INT: i32 = raw::BFLD_INT as i32;
/// Native field type code reserved for future use.
pub const BFLD_RFU0: i32 = raw::BFLD_RFU0 as i32;
/// Native `BFLD_PTR` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_PTR: i32 = raw::BFLD_PTR as i32;
/// Native `BFLD_UBF` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_UBF: i32 = raw::BFLD_UBF as i32;
/// Native `BFLD_VIEW` field type code; use [`crate::UbfFieldType`] for typed UBF access.
pub const BFLD_VIEW: i32 = raw::BFLD_VIEW as i32;
/// Maximum native field type code.
pub const BFLD_MAX: i32 = raw::BFLD_MAX as i32;
/// Invalid field identifier.
pub const BBADFLDID: i32 = raw::NDRX_RS_BBADFLDID;
/// Initial field identifier for native field iteration.
pub const BFIRSTFLDID: i32 = raw::NDRX_RS_BFIRSTFLDID;
/// Signed terminator for recursive field/occurrence paths.
pub const BBADFLDOCC: i32 = raw::NDRX_RS_BBADFLDOCC;
/// Maximum number of field/occurrence pairs in a recursive path.
pub const BFLDOCCMAX: i32 = raw::NDRX_RS_BFLDOCCMAX;
/// Map fields from UBF to a VIEW structure.
pub const B_FTOS: i32 = raw::B_FTOS as i32;
/// Map fields from a VIEW structure to UBF.
pub const B_STOF: i32 = raw::B_STOF as i32;
/// Disable UBF/VIEW field mapping.
pub const B_OFF: i32 = raw::B_OFF as i32;
/// Map fields in both UBF/VIEW directions.
pub const B_BOTH: i32 = raw::B_BOTH as i32;
/// Legacy alias for [`UBFFLDMAX`].
pub const BF_LENGTH: usize = raw::BF_LENGTH as usize;
/// Update a UBF buffer when converting from a VIEW.
pub const BUPDATE: i32 = raw::BUPDATE as i32;
/// Outer-join conversion mode; reserved for future use.
pub const BOJOIN: i32 = raw::BOJOIN as i32;
/// Join a UBF buffer when converting from a VIEW.
pub const BJOIN: i32 = raw::BJOIN as i32;
/// Concatenate fields when converting from a VIEW.
pub const BCONCAT: i32 = raw::BCONCAT as i32;
/// Reject native NULL values when reading a VIEW member.
pub const BVACCESS_NOTNULL: i64 = raw::BVACCESS_NOTNULL as i64;
/// Max c field len in struct.
pub const NDRX_VIEW_CNAME_LEN: usize = raw::NDRX_VIEW_CNAME_LEN as usize;
/// Max flags.
pub const NDRX_VIEW_FLAGS_LEN: usize = raw::NDRX_VIEW_FLAGS_LEN as usize;
/// Max len of the null value.
pub const NDRX_VIEW_NULL_LEN: usize = raw::NDRX_VIEW_NULL_LEN as usize;
/// Max len of view name.
pub const NDRX_VIEW_NAME_LEN: usize = raw::NDRX_VIEW_NAME_LEN as usize;
/// Compiled flags len.
pub const NDRX_VIEW_COMPFLAGS_LEN: usize = raw::NDRX_VIEW_COMPFLAGS_LEN as usize;

// Standard-utility error message size.
/// Maximum native NSTD error message length, excluding its terminator.
pub const MAX_ERROR_LEN: usize = raw::MAX_ERROR_LEN as usize;
