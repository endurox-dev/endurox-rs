//! Regression test for reply demultiplexing.
//!
//! Two calls are in flight on one context. `RS_DEMUX_SLOW` is submitted first
//! and lags; `RS_DEMUX_FAST` replies while the slow call's future is parked on
//! the reply fd.
//!
//! Before the demux, whichever future was polled first ran
//! `tpgetrply(own_cd, TPNOBLOCK)`, which pulled the *other* call's reply off the
//! OS queue and into Enduro/X's in-memory queue (`ndrx_add_to_memq`). The reply
//! fd then had nothing left to signal, and the future waiting on that reply
//! hung. With `tpgetrply(TPGETANY)` demultiplexing, every reply is accepted and
//! routed to its own descriptor, so both calls complete.

use endurox_rs::{
    ubf_fields, AtmiCtx, AtmiError, TokioAtmiCtx, TypedBuffer, UbfValue, TPABSOLUTE, TPBLK_NEXT,
    TPNOBLOCK, TPNOREPLY, TPNOTIME,
};
use std::cell::Cell;
use std::time::{Duration, Instant};

/// Generous relative to the server's 600 ms delay, but far below any plausible
/// NDRX_TOUT, so a hang fails the test rather than stalling the suite.
const BUDGET: Duration = Duration::from_secs(20);

async fn call(ctx: &TokioAtmiCtx, svc: &str, payload: &str) -> Result<String, String> {
    let mut req = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("{svc}: tpalloc request failed: {e}"))?;
    req.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String(payload.to_owned()),
        true,
    )
    .map_err(|e| format!("{svc}: failed to set request field: {e}"))?;

    let mut rsp = ctx
        .tpalloc_ubf(4096)
        .map_err(|e| format!("{svc}: tpalloc response failed: {e}"))?;

    ctx.tpcall(svc, &req, &mut rsp, 0)
        .await
        .map_err(|e| format!("{svc}: tpcall failed: {e}"))?;

    rsp.bget_string(ubf_fields::T_STRING_2_FLD, 0)
        .map_err(|e| format!("{svc}: failed to read response field: {e}"))
}

async fn run() -> Result<(), String> {
    let ctx = AtmiCtx::new().map_err(|e| format!("failed to create AtmiCtx: {e}"))?;
    ctx.tpinit().map_err(|e| format!("tpinit failed: {e}"))?;
    let ctx = ctx
        .into_tokio()
        .map_err(|e| format!("into_tokio failed: {e}"))?;

    let started = Instant::now();

    // SLOW is listed first so it is polled first and parks first.
    let (slow, fast) = tokio::join!(
        call(&ctx, "RS_DEMUX_SLOW", "A"),
        call(&ctx, "RS_DEMUX_FAST", "B"),
    );

    let elapsed = started.elapsed();
    let slow = slow?;
    let fast = fast?;

    if slow != "SLOW:A" {
        return Err(format!("unexpected slow response `{slow}`"));
    }
    if fast != "FAST:B" {
        return Err(format!("unexpected fast response `{fast}`"));
    }
    if elapsed > BUDGET {
        return Err(format!(
            "calls completed but took {elapsed:.2?}, over budget"
        ));
    }

    queue_pressure(&ctx)
        .await
        .map_err(|e| format!("queue pressure: {e}"))?;

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    println!("demux ok: both replies routed in {elapsed:.2?}");
    Ok(())
}

/// Occupy both workers, then fill their shared request queue with fast calls.
/// Only the first two requests sleep; draining the filler messages is quick.
fn fill_queue(ctx: &TokioAtmiCtx, request: &TypedBuffer<'_>) -> Result<(), AtmiError> {
    for _ in 0..2 {
        ctx.try_tpacall("RS_DEMUX_HOLD", request, TPNOREPLY | TPNOTIME)?;
    }
    for _ in 0..1000 {
        match ctx.try_tpacall("RS_DEMUX_FAST", request, TPNOREPLY | TPNOTIME) {
            Err(error) if error.code == AtmiError::TPEBLOCK => return Ok(()),
            Err(error) => return Err(error),
            Ok(_) => (),
        }
    }
    Err(AtmiError::new(
        AtmiError::TPEINVAL,
        "could not fill the request queue",
    ))
}

async fn queue_pressure(ctx: &TokioAtmiCtx) -> Result<(), AtmiError> {
    let request = ctx.tpalloc_ubf(1024)?.into_inner();
    let mut response = ctx.tpalloc_ubf(4096)?.into_inner();

    // A nonblocking send which succeeds must still await a delayed reply.
    ctx.tpcall("RS_DEMUX_SLOW", &request, &mut response, TPNOBLOCK)
        .await?;

    fill_queue(ctx, &request)?;
    let error = ctx
        .tpcall("RS_DEMUX_FAST", &request, &mut response, TPNOBLOCK)
        .await
        .expect_err("full queue must fail immediately with TPNOBLOCK");
    assert_eq!(error.code, AtmiError::TPEBLOCK);

    // Both APIs must yield while the request queue is full. A heartbeat on the
    // same executor thread must run before either submission completes.
    let beat = Cell::new(false);
    let (call, submitted, ()) = tokio::join!(
        async {
            let result = ctx
                .tpcall("RS_DEMUX_FAST", &request, &mut response, 0)
                .await;
            assert!(beat.get(), "tpcall prevented the heartbeat from running");
            result
        },
        async {
            ctx.tpsprio(41, 0)?; // Default 50 + relative 41 = absolute 91.
            let result = ctx.tpacall("RS_DEMUX_FAST", &request, 0).await;
            assert!(beat.get(), "tpacall prevented the heartbeat from running");
            if result.is_ok() {
                assert_eq!(ctx.tpgprio()?, 91);
            }
            result
        },
        async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            ctx.tpsprio(23, TPABSOLUTE).unwrap();
            beat.set(true);
        },
    );
    call?;
    assert!(beat.get());
    let mut cd = submitted?;
    ctx.tpacall("RS_DEMUX_FAST", &request, TPNOREPLY).await?;
    assert_eq!(ctx.tpgprio()?, 23);
    ctx.tpgetrply(&mut cd, &mut response, 0).await?;

    fill_queue(ctx, &request)?;
    // Cancelling a waiting send must not retain a descriptor or submit later.
    assert!(tokio::time::timeout(
        Duration::from_millis(25),
        ctx.tpacall("RS_DEMUX_FAST", &request, 0)
    )
    .await
    .is_err());
    ctx.tpsblktime(1, TPBLK_NEXT)?;
    let start = Instant::now();
    let error = ctx
        .tpacall("RS_DEMUX_FAST", &request, 0)
        .await
        .expect_err("one-shot timeout must expire before the queue opens");
    assert_eq!(error.code, AtmiError::TPETIME);
    assert!(start.elapsed() < Duration::from_millis(1600));

    // The preceding timeout must not leak into a TPNOTIME send. A no-reply
    // request returns zero, with no reply slot allocated by the adapter.
    let cd = ctx
        .tpacall_async("RS_DEMUX_FAST", &request, TPNOREPLY | TPNOTIME)
        .await?;
    assert_eq!(cd, 0);
    ctx.tpcall("RS_DEMUX_FAST", &request, &mut response, 0)
        .await?;

    // More calls than either queue's capacity. The send loop must drain replies
    // while submitting, or the service and caller can block each other's queues.
    let mut calls = Vec::new();
    for _ in 0..32 {
        match ctx.tpacall("RS_DEMUX_FAST", &request, 0).await {
            Ok(cd) => calls.push(cd),
            // Draining native replies can release native descriptors before
            // the Rust caller has collected them. The existing demux rejects
            // reuse with TPELIMIT; collect the older replies without resending
            // a request whose submission may already have reached the service.
            Err(error) if error.code == AtmiError::TPELIMIT => break,
            Err(error) => return Err(error),
        }
    }
    for mut cd in calls {
        ctx.tpgetrply(&mut cd, &mut response, 0).await?;
    }
    println!("queue pressure ok: yielding sends, nonblocking flags, cancellation and deadlines");
    Ok(())
}

/// `AtmiCtx` is `!Sync`, so its futures are `!Send`; a current-thread runtime is
/// required. It also makes the interleaving deterministic: exactly one thread
/// polls both calls, so the fast reply necessarily arrives while the slow
/// future is parked.
#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Belt and braces: if the demux regresses, the futures park forever and no
    // amount of waiting helps. Fail loudly instead of hanging the CI job.
    let outcome = match tokio::time::timeout(BUDGET, run()).await {
        Ok(outcome) => outcome,
        Err(_) => Err(format!(
            "timed out after {BUDGET:?} -- a reply was accepted but never routed \
             to its call descriptor"
        )),
    };

    if let Err(err) = outcome {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
