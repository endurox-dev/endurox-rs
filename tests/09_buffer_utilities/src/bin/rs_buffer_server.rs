use endurox_rs::*;

fn echo(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let data = svc.take_data().expect("request");
    let result = (|| -> AtmiResult<()> {
        let exported = ctx.tpexport(&data, TPEX_STRING)?;
        assert!(exported.len() > 65536);
        let imported = ctx.tpimport(&exported, TPEX_STRING)?;
        assert_eq!(ctx.tpbufcmp(Some(&data), Some(&imported), 0)?, 0);
        Ok(())
    })();
    let status = match result {
        Ok(()) => TpReturnStatus::Success,
        Err(e) => {
            eprintln!("TESTERROR: buffer service: {e}");
            TpReturnStatus::Fail
        }
    };
    ctx.tpreturn(status, 0, data, 0);
}
fn init(ctx: &AtmiCtx, _: &[String]) -> AtmiResult<()> {
    ctx.tpadvertise("RSBUFFER", echo)
}
fn done(_: &AtmiCtx) {}

fn main() {
    let ctx = AtmiCtx::new().expect("server context");
    if let Err(error) = ctx.tp_run(ServerHooks::new(init).done(done)) {
        eprintln!("TESTERROR: server: {error}");
        std::process::exit(1);
    }
}
