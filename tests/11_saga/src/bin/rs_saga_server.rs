use endurox_rs::{ubf_fields as f, *};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

type Records = HashMap<String, (i8, Vec<u8>)>;
static RECORDS: OnceLock<Mutex<Records>> = OnceLock::new();

fn register(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let mut data = TypedUbf::from_typed(svc.take_data().expect("request")).expect("UBF");
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut ctl = TpSagaCtl::default();
        ctl.set_timeout(30)?
            .set_saga_name("RS")?
            .set_saga_key(&data.bget_string(f::T_STRING_FLD, 0)?)?
            .set_content_type("application/octet-stream")?
            .set_payload(&[0, 1, 255])?
            .set_commit_url("xatmi://RSCOMMIT")?
            .set_rollback_url("xatmi://RSROLLBACK")?;
        ctx.tpsagaenlist(&mut ctl, 0)?;
        assert_eq!(ctx.tpsagaenlist(&mut ctl, 0).unwrap_err().code, TPEPROTO);
        ctx.tpsagaend(&mut ctl, 0)?;
        data.bchg(f::T_STRING_FLD, 0, ctl.branch_xid(), false)?;
        data.bchg(f::T_STRING_FLD, 1, ctx.tpxid_to_string(&ctl.xid())?, false)?;
        Ok(())
    })();
    let status = match result {
        Ok(()) => TpReturnStatus::Success,
        Err(e) => {
            eprintln!("TESTERROR: registration: {e}");
            TpReturnStatus::Fail
        }
    };
    ctx.tpreturn_ubf(status, 0, data, 0);
}

fn action(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let data = TypedUbf::from_typed(svc.take_data().expect("callback")).expect("UBF");
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let branch = data.bget_string(TMSAGA_BRXID, 0)?;
        let action = data.bget_char(TMSAGA_ACTION, 0)?;
        let payload = data.bget_bytes(TMSAGA_DATA, 0)?;
        RECORDS
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .entry(branch)
            .or_insert((action, payload));
        Ok(())
    })();
    let status = match result {
        Ok(()) => TpReturnStatus::Success,
        Err(e) => {
            eprintln!("TESTERROR: callback: {e}");
            TpReturnStatus::Fail
        }
    };
    ctx.tpreturn_ubf(status, 0, data, 0);
}

fn state(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let mut data = TypedUbf::from_typed(svc.take_data().expect("query")).expect("UBF");
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let branch = data.bget_string(f::T_STRING_FLD, 0)?;
        let record = RECORDS.get().unwrap().lock().unwrap().remove(&branch);
        data.bchg(f::T_LONG_FLD, 0, i64::from(record.is_some()), false)?;
        if let Some((action, payload)) = record {
            data.bchg(f::T_CHAR_FLD, 0, action, false)?;
            data.bchg(f::T_CARRAY_FLD, 0, payload, true)?;
        }
        Ok(())
    })();
    let status = match result {
        Ok(()) => TpReturnStatus::Success,
        Err(e) => {
            eprintln!("TESTERROR: state: {e}");
            TpReturnStatus::Fail
        }
    };
    ctx.tpreturn_ubf(status, 0, data, 0);
}
fn init(ctx: &AtmiCtx, _: &[String]) -> AtmiResult<()> {
    RECORDS.get_or_init(|| Mutex::new(HashMap::new()));
    ctx.tpopen()?;
    ctx.tpadvertise("RSREGISTER", register)?;
    ctx.tpadvertise("RSCOMMIT", action)?;
    ctx.tpadvertise("RSCOMMIT2", action)?;
    ctx.tpadvertise("RSROLLBACK", action)?;
    ctx.tpadvertise("RSSTATE", state)
}
fn done(ctx: &AtmiCtx) {
    assert!(RECORDS.get().unwrap().lock().unwrap().is_empty());
    ctx.tpclose().expect("close RM");
}

fn main() {
    let ctx = AtmiCtx::new().expect("server context");
    if let Err(error) = ctx.tp_run(ServerHooks::new(init).done(done)) {
        eprintln!("TESTERROR: server: {error}");
        std::process::exit(1);
    }
}
