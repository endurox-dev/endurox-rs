use endurox_rs::AtmiCtx;


#[test]
fn atmictx_init_integration() {

    let mut ctx = AtmiCtx::new();
    assert!(ctx.tpinit().is_ok());

    endurox_rs::ndrx_error!(ctx, "Context created...");

    assert!(ctx.tpterm().is_ok());
}

