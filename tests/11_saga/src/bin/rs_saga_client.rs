#[path = "../../../common/test_duration.rs"]
mod test_duration;
use std::time::Instant;

use endurox_rs::{ubf_fields as f, *};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;
    ctx.tpopen()?;
    let duration = test_duration::test_duration();
    let started = Instant::now();
    let mut iterations = 0u64;
    while iterations < 3 || iterations % 3 != 0 || started.elapsed() < duration {
        let iteration = iterations;
        ctx.tpbegin(60, TPSAGALRA)?;
        let mut request = ctx.tpalloc_ubf(1024)?;
        request.bchg(f::T_STRING_FLD, 0, format!("rust-{iteration}"), false)?;
        let mut reply = ctx.tpalloc_carray(&[])?;
        ctx.tpcall("RSREGISTER", &request, &mut reply, 0)?;
        let reply = TypedUbf::from_typed(reply)?;
        let branch = reply.bget_string(f::T_STRING_FLD, 0)?;
        let global = reply.bget_string(f::T_STRING_FLD, 1)?;
        let xid = ctx.tpxidfromstring(&global)?;
        assert_eq!(ctx.tpxidtostring(&xid)?, global);
        let mut status = TpTxStatus::default();
        status.set_flags(TPTXS_BRXID)?.set_branch_xid(&branch)?;
        ctx.tptxstatus(&mut status, 0)?;
        assert_eq!(status.status(), TPTXS_STATUS_ACTIVE);
        status.set_timeout(90)?;
        ctx.tptxrenew(&mut status, 0)?;
        assert!(status.timeout_left() > 0 && status.timeout_left() <= 30);
        status.set_xid(&xid).set_flags(TPTXS_XID)?;
        ctx.tptxstatus(&mut status, 0)?;
        assert_eq!(status.status(), TPTXS_STATUS_ACTIVE);
        let tranid = ctx.tpsuspend(0)?;
        // A participant joins by the global XID only; it cannot complete.
        let rbonly = iteration % 6 == 0;
        let participant = ctx.tptranidfromxid(&global)?;
        ctx.tpresume(&participant, 0)?;
        assert_eq!(ctx.tpcommit(0).unwrap_err().code, TPEPROTO);
        assert_eq!(ctx.tpabort(0).unwrap_err().code, TPEPROTO);
        ctx.tpsuspend(if rbonly { TPTXABORTONLY } else { 0 })?;
        assert_eq!(ctx.tptranidfromxid(&branch).unwrap_err().code, TPEINVAL);
        status.set_tranid(&tranid).set_flags(TPTXS_TRANID)?;
        ctx.tptxstatus(&mut status, 0)?;
        let mut joined = TpSagaCtl::default();
        joined
            .set_xid(&xid)
            .set_flags(TPSAGA_NOLOCK)?
            .set_saga_name("RS")?
            .set_saga_key("external")?
            .set_commit_url("xatmi://RSCOMMIT")?
            .set_payload(&[1, 0, 2])?;
        ctx.tpsagaenlist(&mut joined, TPSAGA_JOINXID)?;
        let joined_branch = joined.branch_xid();
        ctx.tpsagaenlist(&mut joined, TPSAGA_JOINXID | TPSAGA_UPDATE)?;
        assert_eq!(joined.branch_xid(), joined_branch);
        ctx.tpsagaleave(&mut joined, 0)?;
        ctx.tpresume(&tranid, 0)?;
        let mut ctl = TpSagaCtl::default();
        ctl.set_branch_xid(&branch)?.set_payload(&[9, 0, 8])?;
        ctx.tpsagaget(&mut ctl, 0)?;
        assert_eq!(ctl.saga_name(), "RS");
        assert_eq!(ctl.timeout(), 30);
        assert_eq!(ctl.payload(), &[9, 0, 8]);
        ctl.set_payload(&vec![255; NDRX_SAGA_PAYLOAD_MAX])?;
        assert_eq!(
            ctl.set_payload(&vec![0; NDRX_SAGA_PAYLOAD_MAX + 1])
                .err()
                .unwrap()
                .code,
            TPEINVAL
        );
        // Moving the control must leave the native payload pointer valid.
        let mut moved = Box::new(ctl);
        ctx.tpsagaupdate(&mut moved, 0)?;
        moved
            .set_commit_url("xatmi://RSCOMMIT2")?
            .set_payload(&[5])?;
        ctx.tpsagaupdate(&mut moved, TPSAGA_URLSONLY)?;
        ctx.tpsagaget(&mut moved, 0)?;
        assert_eq!(moved.commit_url(), "xatmi://RSCOMMIT2");
        assert_eq!(moved.payload(), &[5]);
        let removed = iteration % 3 == 2;
        if removed {
            ctx.tpsagaleave(&mut moved, 0)?;
        }
        if iteration % 3 == 1 {
            ctx.tpabort(0)?;
        } else if rbonly {
            assert_eq!(ctx.tpcommit(0).unwrap_err().code, TPEABORT);
        } else {
            ctx.tpcommit(0)?;
        }
        request.bchg(f::T_STRING_FLD, 0, branch, false)?;
        let mut response = ctx.tpalloc_carray(&[])?;
        ctx.tpcall("RSSTATE", &request, &mut response, 0)?;
        let response = TypedUbf::from_typed(response)?;
        assert_eq!(response.bget_long(f::T_LONG_FLD, 0)?, i64::from(!removed));
        if !removed {
            let expected = if iteration % 3 == 1 || rbonly {
                NDRX_SAGA_ACTION_ROLLBACK
            } else {
                NDRX_SAGA_ACTION_COMMIT
            };
            assert_eq!(response.bget_char(f::T_CHAR_FLD, 0)? as u8, expected);
            assert_eq!(
                response.bget_bytes(f::T_CARRAY_FLD, 0)?,
                vec![255; NDRX_SAGA_PAYLOAD_MAX]
            );
        }
        iterations += 1;
    }
    println!(
        "Completed {} iterations in {:.3}s",
        iterations,
        started.elapsed().as_secs_f64()
    );
    ctx.tpclose()?;
    ctx.tpterm()?;
    Ok(())
}
