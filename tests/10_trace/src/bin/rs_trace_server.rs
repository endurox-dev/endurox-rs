use endurox_rs::{ubf_fields as f, *};

fn traced(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let mut data = TypedUbf::from_typed(svc.take_data().expect("request")).expect("UBF");
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let parent = ctx.tptracecontextget(0)?;
        assert_eq!(parent[3..35], data.bget_string(f::T_STRING_FLD, 0)?[3..35]);
        assert_eq!(ctx.tptracestateget(0)?, "rs=client");
        ctx.tpcontexttagset("request", "server", TPCTXTAG_THREAD | TPCTXTAG_BOTH)?;
        let span = ctx.tptracespanbegin("server-work", Some(&data), TPTRACE_F_ACTIVE)?;
        ctx.tptraceevent("server-event", Some(&data), 0)?;
        ctx.tptracestateset("rs=server", 0)?;
        data.bchg(f::T_STRING_FLD, 1, ctx.tptracecontextget(0)?, false)?;
        data.bchg(f::T_STRING_FLD, 2, ctx.tptracestateget(0)?, false)?;
        ctx.tptracespanend(span, TPTRACE_STATUS_OK, Some(&data), 0)?;
        ctx.tpcontexttagclearall(TPCTXTAG_THREAD | TPCTXTAG_BOTH)?;
        Ok(())
    })();
    let status = match result {
        Ok(()) => TpReturnStatus::Success,
        Err(e) => {
            eprintln!("TESTERROR: trace service: {e}");
            TpReturnStatus::Fail
        }
    };
    ctx.tpreturn_ubf(status, 0, data, 0);
}
fn init(ctx: &AtmiCtx, _: &[String]) -> AtmiResult<()> {
    ctx.tpadvertise("RSTRACE", traced)
}
fn done(_: &AtmiCtx) {}

fn main() {
    let ctx = AtmiCtx::new().expect("server context");
    if let Err(error) = ctx.tp_run(ServerHooks::new(init).done(done)) {
        eprintln!("TESTERROR: server: {error}");
        std::process::exit(1);
    }
}
