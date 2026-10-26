use endurox_rs::*;

fn enlist(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let data = svc.take_data().expect("request");
    let result = (|| -> AtmiResult<()> {
        ctx.tprmstart("B", 0)?;
        // These opaque fixture-owned addresses are compared, never dereferenced.
        let a = unsafe { ctx.tpgetconnrm(1)? };
        let b = unsafe { ctx.tpgetconnrm(2)? };
        assert_ne!(a, b);
        ctx.tprmend("B", 0)?;
        Ok(())
    })();
    let status = match result {
        Ok(()) => TpReturnStatus::Success,
        Err(e) => {
            eprintln!("TESTERROR: manual enlistment: {e}");
            TpReturnStatus::Fail
        }
    };
    ctx.tpreturn(status, 0, data, 0);
}
fn init(ctx: &AtmiCtx, _: &[String]) -> AtmiResult<()> {
    ctx.tpopen2(TPTXOPENALL)?;
    ctx.tpadvertise("RSRM", enlist)
}
fn done(ctx: &AtmiCtx) {
    ctx.tpclose().expect("close RMs");
}

fn main() {
    let ctx = AtmiCtx::new().expect("server context");
    if let Err(error) = ctx.tp_run(ServerHooks::new(init).done(done)) {
        eprintln!("TESTERROR: server: {error}");
        std::process::exit(1);
    }
}
