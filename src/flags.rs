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

/// Maximum Saga profile name length in `[@saga/<name>]`, excluding NUL.
pub const NDRX_SAGA_NAME_MAX: usize = raw::NDRX_SAGA_NAME_MAX as usize;

/// Maximum raw bytes of the Saga payload
pub const NDRX_SAGA_PAYLOAD_MAX: usize = raw::NDRX_SAGA_PAYLOAD_MAX as usize;

/// Maximum payload MIME type length, excluding NUL
pub const NDRX_SAGA_MIME_MAX: usize = raw::NDRX_SAGA_MIME_MAX as usize;

/// Maximum length of a TPSAGACTL branch URL, excluding NUL
pub const NDRX_SAGA_URL_MAX: usize = raw::NDRX_SAGA_URL_MAX as usize;

/// Maximum Saga idempotency key length (TPSAGACTL.saga_key), excluding NUL; as a URL, so that
/// a participant URL may be the key
pub const NDRX_SAGA_KEY_MAX: usize = raw::NDRX_SAGA_KEY_MAX as usize;

/// TMSAGA_ACTION value: the commit action is requested
pub const NDRX_SAGA_ACTION_COMMIT: u8 = raw::NDRX_SAGA_ACTION_COMMIT;

/// TMSAGA_ACTION value: the rollback action is requested
pub const NDRX_SAGA_ACTION_ROLLBACK: u8 = raw::NDRX_SAGA_ACTION_ROLLBACK;

/// TMSAGA_ACTION value: the forget action after a heuristic result
pub const NDRX_SAGA_ACTION_FORGET: u8 = raw::NDRX_SAGA_ACTION_FORGET;

/// TMSAGA_ACTION value: the after action, notifies the final outcome
pub const NDRX_SAGA_ACTION_AFTER: u8 = raw::NDRX_SAGA_ACTION_AFTER;

/// TMSAGA_ACTION value: status query of the accepted action given in TMSAGA_STS_ACTION,
/// TMSAGA_URL is the status URL
pub const NDRX_SAGA_ACTION_STATUS: u8 = raw::NDRX_SAGA_ACTION_STATUS;

/// tpurcode with TPFAIL of a commit or rollback action or status query: the action is
/// accepted and completes asynchronously; TMSAGA_STS_URL of the reply may give the status URL
pub const NDRX_SAGA_URC_ACCEPTED: i64 = raw::NDRX_SAGA_URC_ACCEPTED as i64;

/// tpurcode with TPFAIL of a status query: the participant does not know the action, it is
/// sent again
pub const NDRX_SAGA_URC_UNKNOWN: i64 = raw::NDRX_SAGA_URC_UNKNOWN as i64;

/// TMSAGA_OUTCOME of the after action: committed
pub const NDRX_SAGA_OUTCOME_CLOSED: &str = crate::dlm::native_text(raw::NDRX_SAGA_OUTCOME_CLOSED);

/// TMSAGA_OUTCOME of the after action: rolled back
pub const NDRX_SAGA_OUTCOME_CANCELLED: &str =
    crate::dlm::native_text(raw::NDRX_SAGA_OUTCOME_CANCELLED);

/// TMSAGA_OUTCOME of the after action: committed with heuristic result
pub const NDRX_SAGA_OUTCOME_FAILED_TO_CLOSE: &str =
    crate::dlm::native_text(raw::NDRX_SAGA_OUTCOME_FAILED_TO_CLOSE);

/// TMSAGA_OUTCOME of the after action: rolled back with heuristic result
pub const NDRX_SAGA_OUTCOME_FAILED_TO_CANCEL: &str =
    crate::dlm::native_text(raw::NDRX_SAGA_OUTCOME_FAILED_TO_CANCEL);

/// Decoded branch XID bytes ahead of the global XID: version, RMID, BTID
pub const NDRX_SAGA_BRXID_HDR: usize = raw::NDRX_SAGA_BRXID_HDR as usize;

/// Branch XID length, excluding NUL: header and global XID in Base64
pub const NDRX_SAGA_BRXID_MAX: usize = raw::NDRX_SAGA_BRXID_MAX as usize;

/// Registration mode: hold no Saga lock or caller-local Saga state.
pub const TPSAGA_NOLOCK: i64 = raw::TPSAGA_NOLOCK as i64;

/// Register using the global XID supplied in the control.
pub const TPSAGA_JOINXID: i64 = raw::TPSAGA_JOINXID as i64;

/// Explicitly replace a registration with the same participant key.
pub const TPSAGA_UPDATE: i64 = raw::TPSAGA_UPDATE as i64;

/// Replace only participant URLs, including during recovery.
pub const TPSAGA_URLSONLY: i64 = raw::TPSAGA_URLSONLY as i64;

/// TPTXSTATUS.xid identifies the txn.
pub const TPTXS_XID: i64 = raw::TPTXS_XID as i64;

/// TPTXSTATUS.branch_xid identifies it.
pub const TPTXS_BRXID: i64 = raw::TPTXS_BRXID as i64;

/// TPTXSTATUS.tranid identifies it.
pub const TPTXS_TRANID: i64 = raw::TPTXS_TRANID as i64;

/// not known by its TM: completed or never existed
pub const TPTXS_STATUS_NOTFOUND: i64 = raw::TPTXS_STATUS_NOTFOUND as i64;

/// transaction is active.
pub const TPTXS_STATUS_ACTIVE: i64 = raw::TPTXS_STATUS_ACTIVE as i64;

/// prepare is in progress.
pub const TPTXS_STATUS_PREPARING: i64 = raw::TPTXS_STATUS_PREPARING as i64;

/// commit decided, completion pending.
pub const TPTXS_STATUS_COMMITTING: i64 = raw::TPTXS_STATUS_COMMITTING as i64;

/// rollback decided, completion pending.
pub const TPTXS_STATUS_ABORTING: i64 = raw::TPTXS_STATUS_ABORTING as i64;

/// committed with heuristic or hazard result, being forgotten
pub const TPTXS_STATUS_COMMIT_HEURISTIC: i64 = raw::TPTXS_STATUS_COMMIT_HEURISTIC as i64;

/// rolled back with heuristic or hazard result, being forgotten
pub const TPTXS_STATUS_ABORT_HEURISTIC: i64 = raw::TPTXS_STATUS_ABORT_HEURISTIC as i64;

/// committed, kept for the Saga after actions
pub const TPTXS_STATUS_COMMITTED: i64 = raw::TPTXS_STATUS_COMMITTED as i64;

/// rolled back, kept for the Saga after actions
pub const TPTXS_STATUS_ABORTED: i64 = raw::TPTXS_STATUS_ABORTED as i64;

/// W3C version-00 traceparent length, excluding its NUL terminator.
pub const NDRX_TRACEPARENT_LEN: usize = raw::NDRX_TRACEPARENT_LEN as usize;

/// Begin a durable LRA transaction; recovery preserves active work until its deadline.
pub const TPSAGALRA: i64 = raw::TPSAGALRA as i64;

/// Process-wide tags with the lowest precedence across all contexts.
pub const TPCTXTAG_PROCESS: i64 = raw::TPCTXTAG_PROCESS as i64;

/// Context-local tags; trace changes update active spans.
pub const TPCTXTAG_THREAD: i64 = raw::TPCTXTAG_THREAD as i64;

/// Context-local tags with highest precedence, retained until explicitly cleared.
pub const TPCTXTAG_NEXT: i64 = raw::TPCTXTAG_NEXT as i64;

/// Destination: log records.
pub const TPCTXTAG_LOG: i64 = raw::TPCTXTAG_LOG as i64;

/// Destination: spans.
pub const TPCTXTAG_TRACE: i64 = raw::TPCTXTAG_TRACE as i64;

/// Apply tags to both logs and traces; the default when no destination is selected.
pub const TPCTXTAG_BOTH: i64 = raw::TPCTXTAG_BOTH as i64;

/// Make the span current in its Enduro/X context: later ATMI calls, log records and child
/// spans use its IDs and tracestate until it ends. Active spans must be ended in LIFO order.
/// Without the flag the span is created as a child of the current context but does not become
/// current.
pub const TPTRACE_F_ACTIVE: i64 = raw::TPTRACE_F_ACTIVE as i64;

/// Ignore the current context and start a new trace: new trace ID, root sampling decision by
/// the provider, no inherited tracestate.
pub const TPTRACE_F_NEW_ROOT: i64 = raw::TPTRACE_F_NEW_ROOT as i64;

/// Span status not set.
pub const TPTRACE_STATUS_UNSET: i32 = raw::TPTRACE_STATUS_UNSET as i32;

/// Span completed successfully.
pub const TPTRACE_STATUS_OK: i32 = raw::TPTRACE_STATUS_OK as i32;

/// Span failed.
pub const TPTRACE_STATUS_ERROR: i32 = raw::TPTRACE_STATUS_ERROR as i32;

/// Propagated tracestate bytes.
pub const NDRX_TRACE_STATE_MAX: usize = raw::NDRX_TRACE_STATE_MAX as usize;

/// Native scripting engine initialization string capacity, including NUL.
pub const NDRX_SCR_INITSTRING_BUFSZ: usize = raw::NDRX_SCR_INITSTRING_BUFSZ as usize;

/// Saga profile name in callback requests (STRING).
pub const TMSAGA_NAME: i32 = raw::NDRX_RS_TMSAGA_NAME as i32;

/// Configured Saga DLM space (STRING).
pub const TMSAGA_DLMSPACE: i32 = raw::NDRX_RS_TMSAGA_DLMSPACE as i32;

/// Modified Base64 participant branch identifier (STRING).
pub const TMSAGA_BRXID: i32 = raw::NDRX_RS_TMSAGA_BRXID as i32;

/// Registered callback payload bytes (CARRAY).
pub const TMSAGA_DATA: i32 = raw::NDRX_RS_TMSAGA_DATA as i32;

/// Registered payload content type (STRING).
pub const TMSAGA_MIME: i32 = raw::NDRX_RS_TMSAGA_MIME as i32;

/// Requested action, one of NDRX_SAGA_ACTION_* (CHAR).
pub const TMSAGA_ACTION: i32 = raw::NDRX_RS_TMSAGA_ACTION as i32;

/// Current callback endpoint (STRING).
pub const TMSAGA_URL: i32 = raw::NDRX_RS_TMSAGA_URL as i32;

/// Participant idempotency key (STRING).
pub const TMSAGA_KEY: i32 = raw::NDRX_RS_TMSAGA_KEY as i32;

/// Commit callback endpoint (STRING).
pub const TMSAGA_CMT_URL: i32 = raw::NDRX_RS_TMSAGA_CMT_URL as i32;

/// Compensation callback endpoint (STRING).
pub const TMSAGA_RBK_URL: i32 = raw::NDRX_RS_TMSAGA_RBK_URL as i32;

/// Status query endpoint; an accepted action may return a new URL (STRING).
pub const TMSAGA_STS_URL: i32 = raw::NDRX_RS_TMSAGA_STS_URL as i32;

/// Forget callback endpoint (STRING).
pub const TMSAGA_FGT_URL: i32 = raw::NDRX_RS_TMSAGA_FGT_URL as i32;

/// Final outcome notification endpoint (STRING).
pub const TMSAGA_AFT_URL: i32 = raw::NDRX_RS_TMSAGA_AFT_URL as i32;

/// Final outcome, one of NDRX_SAGA_OUTCOME_* (STRING).
pub const TMSAGA_OUTCOME: i32 = raw::NDRX_RS_TMSAGA_OUTCOME as i32;

/// Action whose asynchronous result is queried (CHAR).
pub const TMSAGA_STS_ACTION: i32 = raw::NDRX_RS_TMSAGA_STS_ACTION as i32;
/// Optional Saga registration lifetime in seconds; zero adds no limit.
pub const TMSAGA_TIMEOUT: i32 = raw::NDRX_RS_TMSAGA_TIMEOUT as i32;

/// Maximum base58-encoded DLM client identifier length, excluding NUL.
pub const NDRX_B58_CIDMAX: usize = raw::NDRX_B58_CIDMAX as usize;
