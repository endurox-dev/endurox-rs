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
        // Durable LRA metadata
        status
            .set_client_id("rust-client")?
            .set_lra_url("http://127.0.0.1:1/lra-coordinator")?;
        ctx.tptxsetinfo(&mut status, 0)?;
        assert_eq!(status.client_id(), "rust-client");
        assert_eq!(status.lra_url(), "http://127.0.0.1:1/lra-coordinator");
        assert!(status.is_lra() && !status.is_recovering());
        assert!(0 < status.start_time() && status.start_time() < status.finish_time());
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
        // a data-only update returns the replaced payload
        joined.set_payload(&[3])?;
        ctx.tpsagaenlist(
            &mut joined,
            TPSAGA_JOINXID | TPSAGA_UPDATE | TPSAGA_DATAONLY,
        )?;
        assert_eq!(joined.branch_xid(), joined_branch);
        assert_eq!(joined.previous_payload(), &[1, 0, 2]);
        ctx.tpsagaleave(&mut joined, TPSAGA_BYKEY)?;
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
        // closes that succeed replace the participant data at completion
        let final_data = iteration % 4 < 2 && iteration % 3 == 0 && !rbonly;
        if iteration % 4 < 2 {
            // coordinator completion by global XID, outside the transaction
            ctx.tpsuspend(0)?;
            let mut selector = TpTxStatus::default();
            selector.set_flags(TPTXS_XID)?.set_xid(&xid);
            let abort = iteration % 3 == 1;
            let result = if final_data {
                moved.set_payload(&[7, 0, 7])?;
                ctx.tptxcomplete(&selector, Some(moved.as_mut()), 0)
            } else {
                ctx.tptxcomplete(&selector, None, if abort { TPTXC_ABORT } else { 0 })
            };
            if rbonly && !abort {
                assert_eq!(result.unwrap_err().code, TPEABORT);
            } else {
                result?;
            }
        } else if iteration % 3 == 1 {
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
            let payload = if final_data {
                vec![7, 0, 7]
            } else {
                vec![255; NDRX_SAGA_PAYLOAD_MAX]
            };
            assert_eq!(response.bget_bytes(f::T_CARRAY_FLD, 0)?, payload);
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
