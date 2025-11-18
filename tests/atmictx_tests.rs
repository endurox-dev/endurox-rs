use endurox_rs::AtmiCtx;
use endurox_rs::TypedUbf;
use endurox_rs::TypedBuffer;
use endurox_rs::UbfValue;

#[test]
fn atmictx_init_integration() {

    // new() now returns Result<Self, AtmiError>
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    // tpinit() returns AtmiResult<()>
    ctx.tpinit().expect("tpinit failed");

    endurox_rs::ndrx_error!(ctx, "Context created...");

    // tpterm() returns AtmiResult<()>
    ctx.tpterm().expect("tpterm failed");
}

#[test]
fn tpalloc_generic_and_cast_to_ubf() {
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    // generic typed buffer
    let tbuf: TypedBuffer<'_> = ctx
        .tpalloc("UBF", "", 0)
        .expect("tpalloc failed");

    // “inherit” by casting to TypedUbf
    let ubf: TypedUbf<'_> = TypedUbf::from_typed(tbuf);

    assert!(!ubf.as_ptr().is_null());
    assert!(!ubf.as_ubfh().is_null());

    //ctx.tpterm().expect("tpterm failed");
    ctx.tpinit().expect("Second init shall go OK");
}

#[test]
fn tpalloc_ubf() {
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    endurox_rs::ndrx_error!(ctx, ">>>>> About to alloc UBF...");
    let mut buf = ctx.tpalloc_ubf(1025).expect("Shall Alloc buffer OK");

    buf.bchg(1, 0, UbfValue::Long(5), false).expect("Bchg failed");

    //Move context
    let ctx2 = AtmiCtx::new().expect("failed to create AtmiCtx2");

    /* allocate to new context */
    let buf2 = unsafe {buf.move_to_context(&ctx2) };

    endurox_rs::ndrx_error!(ctx, ">>>>> About to free UBF...");
    drop(buf2);
    drop(ctx);
}
