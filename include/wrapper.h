#include <ndrx_config.h>
#include <ndebug.h>
#include <ondebug.h>
#include <Exfields.h>
#include <xatmi.h>
#include <oatmi.h>
#include <oatmisrv.h>
#include <oatmisrv_integra.h>
#include <ubf.h>
#include <oubf.h>
#include <nerror.h>
#include <nstdutil.h>

/* libatmisrvinteg exports these worker lifecycle hooks but public headers do
 * not declare the variables. Language integrations must install the default
 * hooks when enabling dispatch-thread mode. */
extern int (*ndrx_G_tpsvrthrinit)(int argc, char **argv);
extern void (*ndrx_G_tpsvrthrdone)(void);

/* Bindgen does not evaluate field-table macros containing BFLDID casts. */
enum {
    NDRX_RS_EX_DLM_CMD = EX_DLM_CMD,
    NDRX_RS_EX_DLM_OP = EX_DLM_OP,
    NDRX_RS_EX_DLM_FLAGS = EX_DLM_FLAGS,
    NDRX_RS_EX_DLM_KEY = EX_DLM_KEY,
    NDRX_RS_EX_DLM_EXPIRY = EX_DLM_EXPIRY,
    NDRX_RS_EX_DLM_CID = EX_DLM_CID,
    NDRX_RS_EX_DLM_DATA = EX_DLM_DATA,
    NDRX_RS_EX_DLM_CURDATA = EX_DLM_CURDATA,
    NDRX_RS_EX_DLM_CMDENTRY = EX_DLM_CMDENTRY,
    NDRX_RS_EX_DLM_PID = EX_DLM_PID,
    NDRX_RS_EX_DLM_PID_START = EX_DLM_PID_START,
    NDRX_RS_EX_DLM_NODEID = EX_DLM_NODEID,
    NDRX_RS_EX_DLM_REQCID = EX_DLM_REQCID,
    NDRX_RS_EX_DLM_SPACE = EX_DLM_SPACE,
    NDRX_RS_EX_DLM_DATA_BLOB = EX_DLM_DATA_BLOB,
    NDRX_RS_EX_DLM_CURDATA_BLOB = EX_DLM_CURDATA_BLOB,
    NDRX_RS_EX_DLM_SEQNO = EX_DLM_SEQNO,
    NDRX_RS_EX_DLM_SEQNO_TSEC = EX_DLM_SEQNO_TSEC,
    NDRX_RS_EX_DLM_SEQNO_TUSEC = EX_DLM_SEQNO_TUSEC,
    NDRX_RS_EX_DLM_RVAL1 = EX_DLM_RVAL1,
    NDRX_RS_EX_DLM_LOCK_MODE = EX_DLM_LOCK_MODE,
    NDRX_RS_EX_DLM_KEYTYP = EX_DLM_KEYTYP,
    NDRX_RS_EX_DLM_WAIT_TIME = EX_DLM_WAIT_TIME,
    NDRX_RS_EX_DLM_EVENT = EX_DLM_EVENT,
    NDRX_RS_EX_DLM_EXPIRY_USEC = EX_DLM_EXPIRY_USEC,
    NDRX_RS_EX_DLM_ADDDATA = EX_DLM_ADDDATA,
    NDRX_RS_EX_DLM_CURKEY = EX_DLM_CURKEY,
    NDRX_RS_EX_DLM_INCONCLUSIVE = EX_DLM_INCONCLUSIVE,
    NDRX_RS_EX_DLM_OPFLAGS = EX_DLM_OPFLAGS,
};
