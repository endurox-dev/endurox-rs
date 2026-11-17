use super::endurox_test_env;
use endurox_rs::{AtmiCtx, UbfError, UbfFieldType, ViewValue};
use ubf_fields as f;

#[test]
fn text_roundtrip_preserves_escapes_and_nested_layouts() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut source = ctx.tpalloc_ubf(16384).unwrap();
    let binary: Vec<u8> = (0..=255).collect();
    let string = "line one\n\tline two\\end";
    source
        .bchg(f::T_CARRAY_FLD, 0, binary.clone(), false)
        .unwrap();
    source.bchg(f::T_STRING_FLD, 0, string, false).unwrap();
    source.bchg(f::T_CHAR_FLD, 0, 0i8, false).unwrap();
    let numeric = ctx.bmkfldid_typed(UbfFieldType::String, 100000);
    source.bchg(numeric, 0, "numeric field", false).unwrap();
    let mut child = ctx.tpalloc_ubf(4096).unwrap();
    child.bchg(f::T_STRING_FLD, 0, string, false).unwrap();
    source.bchg(f::T_UBF_FLD, 0, child, false).unwrap();
    let mut view = ctx.tpalloc_view("TESTVIEW1", 1024).unwrap();
    view.bvchg("tlong", 0, 123i64).unwrap();
    view.bvchg("tstring", 0, string).unwrap();
    source.bchg(f::T_VIEW_FLD, 0, view, false).unwrap();

    let text = source.bsprint().unwrap();
    let mut dest = ctx.tpalloc_ubf(16384).unwrap();
    dest.bextread(&text).unwrap();
    assert_eq!(dest.bget_bytes(f::T_CARRAY_FLD, 0).unwrap(), binary);
    assert_eq!(dest.bget_string(f::T_STRING_FLD, 0).unwrap(), string);
    assert_eq!(dest.bget_char(f::T_CHAR_FLD, 0).unwrap(), 0);
    assert_eq!(dest.bget_string(numeric, 0).unwrap(), "numeric field");
    assert_eq!(
        dest.bget_ubf(f::T_UBF_FLD, 0)
            .unwrap()
            .bget_string(f::T_STRING_FLD, 0)
            .unwrap(),
        string
    );
    let view = dest.bget_view(f::T_VIEW_FLD, 0).unwrap();
    assert_eq!(view.bvget_i64("tlong", 0, 0).unwrap(), 123);
    assert_eq!(view.bvget_string("tstring", 0, 0).unwrap(), string);
}

#[test]
fn text_comments_commands_and_errors_never_replay_fields() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buffer = ctx.tpalloc_ubf(4096).unwrap();
    buffer.bextread("").unwrap();
    buffer.bextread("# comment\n").unwrap();
    buffer.bextread("T_STRING_FLD\thello\n# comment\n").unwrap();
    assert_eq!(ctx.boccur(&buffer, f::T_STRING_FLD).unwrap(), 1);
    buffer
        .bextread("+ T_STRING_FLD\tchanged\n= T_STRING_2_FLD\tT_STRING_FLD")
        .unwrap();
    assert_eq!(buffer.bget_string(f::T_STRING_FLD, 0).unwrap(), "changed");
    assert_eq!(buffer.bget_string(f::T_STRING_2_FLD, 0).unwrap(), "changed");
    assert_eq!(ctx.boccur(&buffer, f::T_STRING_FLD).unwrap(), 1);
    assert!(buffer.bextread("T_LONG_FLD\t7\ninvalid line\n").is_err());
    assert_eq!(ctx.boccur(&buffer, f::T_LONG_FLD).unwrap(), 1);
    assert_eq!(buffer.bget_long(f::T_LONG_FLD, 0).unwrap(), 7);
    assert_eq!(
        buffer.bextread("T_STRING_FLD\tx\0y").unwrap_err().code,
        UbfError::BEINVAL
    );
    assert_eq!(ctx.boccur(&buffer, f::T_STRING_FLD).unwrap(), 1);
}

#[test]
fn unsigned_values_are_preserved_or_rejected_before_writing() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buffer = ctx.tpalloc_ubf(4096).unwrap();
    let mut view = ctx.tpalloc_view("TESTVIEW1", 1024).unwrap();
    for value in [32767u16, 32768, 60000, u16::MAX] {
        buffer.bchg(f::T_LONG_FLD, 0, value, false).unwrap();
        assert_eq!(
            buffer.bget_long(f::T_LONG_FLD, 0).unwrap(),
            i64::from(value)
        );
        view.bvchg("tlong", 0, value).unwrap();
        assert_eq!(view.bvget_i64("tlong", 0, 0).unwrap(), i64::from(value));
    }
    buffer.badd(f::T_LONG_FLD, 60000u16, false).unwrap();
    buffer
        .fast_adder()
        .add(f::T_LONG_FLD, u16::MAX, false)
        .unwrap();
    assert_eq!(buffer.bget_long(f::T_LONG_FLD, 1).unwrap(), 60000);
    assert_eq!(buffer.bget_long(f::T_LONG_FLD, 2).unwrap(), 65535);
    for value in [i64::MAX as u64 + 1, u64::MAX] {
        assert_eq!(
            buffer
                .bchg(f::T_LONG_FLD, 0, value, false)
                .unwrap_err()
                .code,
            UbfError::BEINVAL
        );
        assert_eq!(
            buffer.badd(f::T_LONG_FLD, value, false).unwrap_err().code,
            UbfError::BEINVAL
        );
        assert_eq!(
            buffer
                .fast_adder()
                .add(f::T_LONG_FLD, value, false)
                .unwrap_err()
                .code,
            UbfError::BEINVAL
        );
        assert_eq!(
            view.bvchg("tlong", 0, value).unwrap_err().code,
            UbfError::BEINVAL
        );
    }
    #[cfg(target_pointer_width = "64")]
    {
        assert!(buffer.bchg(f::T_LONG_FLD, 0, usize::MAX, false).is_err());
        assert!(view.bvchg("tlong", 0, usize::MAX).is_err());
    }
    assert!(view.bvchg("tlong", 0, ViewValue::Int(i64::MAX)).is_err());
    assert_eq!(buffer.bget_long(f::T_LONG_FLD, 0).unwrap(), 65535);
    assert_eq!(ctx.boccur(&buffer, f::T_LONG_FLD).unwrap(), 3);
    assert_eq!(view.bvget_i64("tlong", 0, 0).unwrap(), 65535);
}

#[test]
fn large_field_and_json_conversions_grow_past_old_limits() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let text = "x".repeat(100000);
    let bytes = vec![0xAB; 100000];
    let mut ubf = ctx.tpalloc_ubf(300000).unwrap();
    ubf.bchg(f::T_STRING_FLD, 0, text.as_str(), false).unwrap();
    ubf.bchg(f::T_CARRAY_FLD, 0, bytes.clone(), false).unwrap();
    assert_eq!(ubf.bget_string(f::T_STRING_FLD, 0).unwrap(), text);
    assert_eq!(ubf.bget_bytes(f::T_CARRAY_FLD, 0).unwrap(), bytes);
    // Each field remains within the native JSON scratch limit. Many escaped
    // occurrences make the total exceed both 64 KiB and the 4x initial estimate.
    ctx.binit(&mut ubf, 300000).unwrap();
    let escaped = "\u{1}".repeat(16000);
    for occ in 0..8 {
        ubf.bchg(f::T_STRING_FLD, occ, escaped.as_str(), false)
            .unwrap();
    }
    let json = ctx.tpubftojson(&ubf).unwrap();
    assert!(json.len() > 65536);
    let mut restored = ctx.tpalloc_ubf(300000).unwrap();
    ctx.tpjsontoubf(&mut restored, &json).unwrap();
    assert_eq!(restored.bget_string(f::T_STRING_FLD, 7).unwrap(), escaped);
    let mut view = ctx
        .tpalloc_view("LARGE_VIEW", ctx.bvsizeof("LARGE_VIEW").unwrap())
        .unwrap();
    // Native VIEW element sizes are signed shorts; the overall layout may
    // exceed 64 KiB, but an individual string/CARRAY cannot exceed 32767.
    view.bvchg("text", 3, &text[..32000]).unwrap();
    view.bvchg("blob", 3, ViewValue::Carray(bytes[..32000].to_vec()))
        .unwrap();
    assert_eq!(view.bvget_string("text", 3, 0).unwrap(), text[..32000]);
    assert_eq!(view.bvget_bytes("blob", 3, 0).unwrap(), bytes[..32000]);
    let mut view = ctx.tpalloc_view("JSON_EXPANSION", 1024).unwrap();
    for i in 0..48 {
        view.bvchg(&format!("long_member_name_for_json_{i:03}"), 0, b'X' as i8)
            .unwrap();
    }
    let json = view.tpviewtojson(0).unwrap();
    assert!(json.len() > 1024);
    assert!(json.contains("long_member_name_for_json_047"));
}
