//! Server for the reply-demux regression test.
//!
//! `RS_DEMUX_SLOW` deliberately lags so that `RS_DEMUX_FAST`'s reply lands on
//! the client's reply queue while the slow call's future is still parked. That
//! is the interleaving that used to strand a waiter.

use endurox_rs::{AtmiCtx, AtmiResult, ServerHooks, TpReturnStatus, TpSvcInfo, UbfValue};
use std::time::Duration;

const SLOW_DELAY: Duration = Duration::from_millis(600);

fn echo_back(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>, tag: &str) {
    let mut ubf = match svc.take_data_ubf() {
        Some(ubf) => ubf,
        None => return,
    };

    let request = ubf
        .bget_string(ubf_fields::T_STRING_FLD, 0)
        .unwrap_or_default();

    if ubf
        .bchg(
            ubf_fields::T_STRING_2_FLD,
            0,
            UbfValue::String(format!("{tag}:{request}")),
            true,
        )
        .is_err()
    {
        ctx.tpreturn_ubf(TpReturnStatus::Fail, 1, ubf, 0);
        return;
    }

    let status = if request == "INPLACE_FAIL" {
        TpReturnStatus::Fail
    } else {
        TpReturnStatus::Success
    };
    if request.starts_with("INPLACE_") {
        ubf.bchg(ubf_fields::T_CARRAY_FLD, 0, vec![0x5Au8; 12000], true)
            .unwrap();
    }
    ctx.tpreturn_ubf(status, 0, ubf, 0);
}

fn rs_demux_slow(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    std::thread::sleep(SLOW_DELAY);
    echo_back(ctx, svc, "SLOW");
}

fn rs_demux_fast(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    echo_back(ctx, svc, "FAST");
}

fn rs_demux_hold(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    std::thread::sleep(Duration::from_secs(2));
    echo_back(ctx, svc, "HOLD");
}

fn rs_demux_init(ctx: &AtmiCtx, _args: &[String]) -> AtmiResult<()> {
    ctx.tpadvertise("RS_DEMUX_SLOW", rs_demux_slow)?;
    ctx.tpadvertise("RS_DEMUX_FAST", rs_demux_fast)?;
    ctx.tpadvertise("RS_DEMUX_HOLD", rs_demux_hold)?;
    ctx.tpadvertise("RS_DEMUX_BYTES", rs_demux_bytes)?;
    Ok(())
}

fn rs_demux_done(_ctx: &AtmiCtx) {}

fn main() {
    let ctx = match AtmiCtx::new() {
        Ok(ctx) => ctx,
        Err(err) => {
            eprintln!("failed to create AtmiCtx: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = ctx.tp_run(ServerHooks::new(rs_demux_init).done(rs_demux_done)) {
        eprintln!("tp_run failed: {err}");
        std::process::exit(1);
    }
}

fn rs_demux_bytes(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    std::thread::sleep(SLOW_DELAY);
    let Some(mut data) = svc.take_data() else {
        return;
    };
    if data.as_bytes() != b"in\0place" {
        ctx.tpreturn(TpReturnStatus::Fail, 1, data, 0);
        return;
    }
    data.set_bytes(&vec![0x33; 12000]).unwrap();
    ctx.tpreturn(TpReturnStatus::Success, 0, data, 0);
}
