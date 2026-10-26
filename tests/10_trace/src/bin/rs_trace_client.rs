#[path = "../../../common/test_duration.rs"]
mod test_duration;
use std::time::Instant;

use endurox_rs::{ubf_fields as f, *};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;
    assert_eq!(ctx.tptracecontextset("bad", 0).unwrap_err().code, TPEINVAL);
    assert_eq!(
        ctx.tpcontexttagset("bad\0key", "v", TPCTXTAG_THREAD)
            .unwrap_err()
            .code,
        TPEINVAL
    );
    let parent = "00-0123456789abcdef0123456789abcdef-0123456789abcdef-01";
    let duration = test_duration::test_duration();
    let started = Instant::now();
    let mut iterations = 0u64;
    while iterations < 1 || started.elapsed() < duration {
        ctx.tptracecontextset(parent, 0)?;
        assert_eq!(ctx.tptracecontextget(0)?, parent);
        ctx.tpcontexttagset("process", "rust", TPCTXTAG_PROCESS | TPCTXTAG_BOTH)?;
        ctx.tpcontexttagset("next", "kept", TPCTXTAG_NEXT | TPCTXTAG_TRACE)?;
        let span = ctx.tptracespanbegin("client-work", None, TPTRACE_F_ACTIVE)?;
        ctx.tptracestateset("rs=client", 0)?;
        let active = ctx.tptracecontextget(0)?;
        let mut request = ctx.tpalloc_ubf(1024)?;
        request.bchg(f::T_STRING_FLD, 0, active.clone(), false)?;
        ctx.tptraceevent("client-event", Some(&request), 0)?;
        let mut reply = ctx.tpalloc_carray(&[])?;
        ctx.tpcall("RSTRACE", &request, &mut reply, 0)?;
        let reply = TypedUbf::from_typed(reply)?;
        assert_eq!(reply.bget_string(f::T_STRING_FLD, 1)?[3..35], active[3..35]);
        assert_eq!(reply.bget_string(f::T_STRING_FLD, 2)?, "rs=server");
        assert_eq!(ctx.tptracestateget(0)?, "rs=client");
        let nested =
            ctx.tptracespanbegin("new-root", None, TPTRACE_F_ACTIVE | TPTRACE_F_NEW_ROOT)?;
        assert_ne!(ctx.tptracecontextget(0)?[3..35], active[3..35]);
        assert_eq!(
            ctx.tptracespanend(span, TPTRACE_STATUS_OK, None, 0)
                .unwrap_err()
                .code,
            TPEPROTO
        );
        ctx.tptracespanend(nested, TPTRACE_STATUS_ERROR, None, 0)?;
        ctx.tptracespanend(span, TPTRACE_STATUS_OK, None, 0)?;
        assert_eq!(
            ctx.tptracespanend(span, TPTRACE_STATUS_OK, None, 0)
                .unwrap_err()
                .code,
            TPEINVAL
        );
        ctx.tpcontexttagclear("next", TPCTXTAG_NEXT | TPCTXTAG_TRACE)?;
        ctx.tpcontexttagclearall(TPCTXTAG_PROCESS | TPCTXTAG_BOTH)?;
        ctx.tptracecontextset("", 0)?;
        assert!(ctx.tptracecontextget(0)?.is_empty());
        iterations += 1;
    }
    println!(
        "Completed {} iterations in {:.3}s",
        iterations,
        started.elapsed().as_secs_f64()
    );
    ctx.tpterm()?;
    Ok(())
}
