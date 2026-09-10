use endurox_rs::{AtmiCtx, ScriptBuffers, ScriptResult, ScriptSlot as S};

#[test]
fn script_buffers_replace_one_alias_and_take_unique_ownership() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buffers = ScriptBuffers::new(&ctx);
    buffers
        .set(S::Input, Some(ctx.tpalloc_carray(b"input").unwrap()))
        .unwrap();
    buffers.alias(S::Param, S::Input).unwrap();
    buffers.alias(S::Output, S::Input).unwrap();
    buffers
        .set(S::Output, Some(ctx.tpalloc_carray(b"other").unwrap()))
        .unwrap();
    assert_eq!(buffers.get(S::Input).unwrap().unwrap().as_bytes(), b"input");
    let taken = buffers.take(S::Param).unwrap().unwrap();
    assert!(buffers.get(S::Input).unwrap().is_none());
    assert!(buffers.get(S::Param).unwrap().is_none());
    assert_eq!(
        buffers.get(S::Output).unwrap().unwrap().as_bytes(),
        b"other"
    );
    drop(buffers);
    assert_eq!(taken.as_bytes(), b"input");
}

#[test]
fn script_buffer_edits_restore_aliases_on_panic_and_preserve_wrong_types() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buffers = ScriptBuffers::new(&ctx);
    buffers
        .set(S::Input, Some(ctx.tpalloc_carray(b"short").unwrap()))
        .unwrap();
    buffers.alias(S::Output, S::Input).unwrap();
    assert!(buffers.edit_ubf(S::Input, |_| Ok(())).is_err());
    assert_eq!(buffers.get(S::Input).unwrap().unwrap().as_bytes(), b"short");
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        buffers.edit(S::Input, |buffer| -> ScriptResult<()> {
            buffer.set_bytes(&vec![7; 32768])?;
            panic!("intentional edit panic");
        })
    }));
    assert!(outcome.is_err());
    assert_eq!(
        buffers.get(S::Output).unwrap().unwrap().as_bytes(),
        vec![7; 32768]
    );
    assert_eq!(buffers.take(S::Output).unwrap().unwrap().len(), 32768);
    assert!(buffers.get(S::Input).unwrap().is_none());
}
