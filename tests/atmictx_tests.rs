use endurox_rs::AtmiCtx;

#[test]
fn atmictx_init_integration() {
    let ctx = AtmiCtx::init();
    assert!(ctx.is_ok());
}

