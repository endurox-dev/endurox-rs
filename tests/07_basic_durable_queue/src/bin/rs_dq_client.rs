use endurox_rs::{ubf_fields, AtmiCtx, TpQCtl, TypedUbf, TPQCORRID, TPQGETBYCORRID};

const QSPACE: &str = "SAMPLESPACE";
const QNAME: &str = "TESTQ";

fn main() {
    let rc = match run() {
        Ok(()) => 0,
        Err(msg) => {
            eprintln!("{msg}");
            1
        }
    };
    std::process::exit(rc);
}

fn run() -> Result<(), String> {
    let scenario = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "enqueue-dequeue".to_string());

    let ctx = AtmiCtx::new().map_err(|e| format!("failed to create AtmiCtx: {e}"))?;
    ctx.tpinit().map_err(|e| format!("tpinit failed: {e}"))?;

    let result = match scenario.as_str() {
        "enqueue-dequeue" => run_enqueue_dequeue(&ctx),
        "corrid" => run_corrid(&ctx),
        "fifo" => run_fifo(&ctx),
        other => Err(format!("unknown scenario `{other}`")),
    };

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    result
}

fn enqueue_str(ctx: &AtmiCtx, value: &str) -> Result<(), String> {
    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc_ubf failed: {e}"))?;
    buf.bchg(ubf_fields::T_STRING_FLD, 0, value, true)
        .map_err(|e| format!("bchg failed: {e}"))?;
    let mut ctl = TpQCtl::default();
    ctx.tpenqueue(QSPACE, QNAME, &mut ctl, &buf, 0)
        .map_err(|e| format!("tpenqueue `{value}` failed: {e}"))
}

fn dequeue_str(ctx: &AtmiCtx) -> Result<String, String> {
    let mut ctl = TpQCtl::default();
    let buf = ctx
        .tpdequeue(QSPACE, QNAME, &mut ctl, 0)
        .map_err(|e| format!("tpdequeue failed: {e}"))?;
    let ubf = TypedUbf::from_typed(buf);
    ubf.bget_string(ubf_fields::T_STRING_FLD, 0)
        .map_err(|e| format!("bget_string failed: {e}"))
}

fn run_enqueue_dequeue(ctx: &AtmiCtx) -> Result<(), String> {
    enqueue_str(ctx, "HELLO-QUEUE")?;
    let val = dequeue_str(ctx)?;
    if val != "HELLO-QUEUE" {
        return Err(format!(
            "enqueue-dequeue: expected `HELLO-QUEUE`, got `{val}`"
        ));
    }
    Ok(())
}

fn run_corrid(ctx: &AtmiCtx) -> Result<(), String> {
    let corrid: [u8; 31] = {
        let mut c = [0u8; 31];
        c[..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        c
    };

    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc_ubf failed: {e}"))?;
    buf.bchg(ubf_fields::T_STRING_FLD, 0, "CORRID-MSG", true)
        .map_err(|e| format!("bchg failed: {e}"))?;

    let mut enq_ctl = TpQCtl::default();
    enq_ctl
        .set_corrid(&corrid)
        .map_err(|e| format!("set_corrid failed: {e}"))?;
    enq_ctl.add_flags(TPQCORRID);
    ctx.tpenqueue(QSPACE, QNAME, &mut enq_ctl, &buf, 0)
        .map_err(|e| format!("tpenqueue (corrid) failed: {e}"))?;

    let mut deq_ctl = TpQCtl::default();
    deq_ctl
        .set_corrid(&corrid)
        .map_err(|e| format!("set_corrid (deq) failed: {e}"))?;
    deq_ctl.add_flags(TPQGETBYCORRID);
    let dequeued = ctx
        .tpdequeue(QSPACE, QNAME, &mut deq_ctl, 0)
        .map_err(|e| format!("tpdequeue (by corrid) failed: {e}"))?;
    let ubf = TypedUbf::from_typed(dequeued);
    let val = ubf
        .bget_string(ubf_fields::T_STRING_FLD, 0)
        .map_err(|e| format!("bget_string (corrid) failed: {e}"))?;

    if val != "CORRID-MSG" {
        return Err(format!("corrid: expected `CORRID-MSG`, got `{val}`"));
    }
    Ok(())
}

fn run_fifo(ctx: &AtmiCtx) -> Result<(), String> {
    let messages = ["FIFO-1", "FIFO-2", "FIFO-3"];
    for msg in &messages {
        enqueue_str(ctx, msg)?;
    }
    for expected in &messages {
        let val = dequeue_str(ctx)?;
        if &val != expected {
            return Err(format!(
                "fifo order: expected `{expected}`, got `{val}`"
            ));
        }
    }
    Ok(())
}
