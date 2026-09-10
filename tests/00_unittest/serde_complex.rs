use endurox_rs::{
    ubf_fields as f, AtmiCtx, EmbeddedUbf, PointerUbf, UbfAdhoc, UbfCarray, UbfDeserialize,
    UbfFieldDeserialize, UbfFieldSerialize, UbfMappedDeserialize, UbfMappedSerialize, UbfSerialize,
    ViewDeserialize, ViewSerialize,
};

#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
struct Child {
    #[ubf(field = f::T_LONG_FLD)]
    id: i64,
    #[ubf(field = f::T_STRING_FLD)]
    text: String,
}
fn child(id: i64) -> Child {
    Child {
        id,
        text: format!("child-{id}"),
    }
}

#[derive(Debug, Clone, PartialEq, ViewSerialize, ViewDeserialize)]
#[view(name = "SERDE_VIEW")]
struct Position {
    id: i64,
    name: String,
    values: Vec<i64>,
    note: Option<String>,
    blob: UbfCarray,
    fixed: [i16; 3],
}
fn position(id: i64) -> Position {
    Position {
        id,
        name: format!("view-{id}"),
        values: vec![id, 0],
        note: Some("note".into()),
        blob: UbfCarray(vec![0, 1, 255]),
        fixed: [0, -2, 32767],
    }
}

#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
struct Branch {
    #[ubf(field = f::T_PTR_FLD, ptr, size = 64)]
    children: Vec<Child>,
    #[ubf(field = f::T_VIEW_FLD, view)]
    position: Position,
}
#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
struct Header {
    #[ubf(field = f::T_LONG_2_FLD, occ = 1)]
    serial: i64,
    #[ubf(field = f::T_SHORT_FLD, occ = 2)]
    flags: [i16; 2],
}
#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
struct Mixed {
    #[ubf(flatten)]
    header: Header,
    // Branch owns BFLD_PTR children, so it must be stored behind a pointer,
    // not inline-embedded.
    #[ubf(field = f::T_PTR_FLD, ptr, size = 64)]
    branch: Branch,
    #[ubf(field = f::T_UBF_2_FLD, nested, occ = 1)]
    inline: Vec<Child>,
    #[ubf(field = f::T_PTR_2_FLD, ptr)]
    optional: Option<Box<Child>>,
    #[ubf(field = f::T_VIEW_2_FLD, view, occ = 1)]
    views: [Position; 2],
    #[ubf(field = f::T_PTR_3_FLD, ptr, view)]
    view_pointer: Option<Position>,
    #[ubf(field = f::T_STRING_9_FLD, default)]
    defaulted: String,
    #[ubf(skip)]
    scratch: usize,
}
fn mixed() -> Mixed {
    Mixed {
        header: Header {
            serial: 12,
            flags: [3, 4],
        },
        branch: Branch {
            children: vec![child(1), child(2)],
            position: position(3),
        },
        inline: vec![child(4), child(5)],
        optional: Some(Box::new(child(6))),
        views: [position(7), position(8)],
        view_pointer: Some(position(9)),
        defaulted: String::new(),
        scratch: 0,
    }
}

#[test]
fn mixed_inline_pointer_and_view_layout_roundtrips() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(128).unwrap();
    let value = mixed();
    buf.ubf_write(&value, true).unwrap();
    assert!(ctx.ubf_has_pointer_fields(&buf).unwrap());
    assert_eq!(buf.ubf_read::<Mixed>().unwrap(), value);
    assert_eq!(ctx.boccur(&buf, f::T_UBF_2_FLD).unwrap(), 3);
    assert_eq!(ctx.boccur(&buf, f::T_VIEW_2_FLD).unwrap(), 3);
    assert_eq!(buf.bget_short(f::T_SHORT_FLD, 2).unwrap(), 3);
    // Non-destructive deserialization remains valid for subsequent reads.
    assert_eq!(buf.ubf_read::<Mixed>().unwrap(), value);
}

#[test]
fn shorter_collections_and_none_remove_owned_occurrences() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(128).unwrap();
    let mut value = mixed();
    buf.ubf_write(&value, true).unwrap();
    value.branch.children.truncate(1);
    value.inline.clear();
    value.optional = None;
    value.view_pointer = None;
    value.views[0].note = None;
    value.views[0].values = vec![0];
    buf.ubf_write(&value, true).unwrap();
    assert_eq!(buf.ubf_read::<Mixed>().unwrap(), value);
    assert_eq!(ctx.boccur(&buf, f::T_UBF_2_FLD).unwrap(), 1);
    assert_eq!(ctx.boccur(&buf, f::T_PTR_2_FLD).unwrap(), 0);
    assert_eq!(ctx.boccur(&buf, f::T_PTR_3_FLD).unwrap(), 0);
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct Tree {
    #[ubf(field = f::T_LONG_FLD)]
    id: i64,
    #[ubf(field = f::T_UBF_FLD, nested)]
    inline: Vec<Tree>,
    #[ubf(field = f::T_PTR_FLD, ptr)]
    next: Option<Box<Tree>>,
}
#[test]
fn recursive_structs_use_both_inline_and_pointer_children() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(128).unwrap();
    let value = Tree {
        id: 1,
        // Inline (embedded UBF) children must be pointer-free.
        inline: vec![Tree {
            id: 2,
            inline: vec![],
            next: None,
        }],
        // A pointer child may itself own both pointer and inline children.
        next: Some(Box::new(Tree {
            id: 3,
            inline: vec![Tree {
                id: 4,
                inline: vec![],
                next: None,
            }],
            next: None,
        })),
    };
    buf.ubf_write(&value, true).unwrap();
    assert_eq!(buf.ubf_read::<Tree>().unwrap(), value);
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct Generic<T> {
    #[ubf(field = f::T_UBF_FLD, nested)]
    values: Vec<T>,
}
#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct GenericScalar<T> {
    #[ubf(field = f::T_LONG_FLD)]
    value: T,
}
#[test]
fn derives_infer_generic_field_bounds() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(128).unwrap();
    let value = Generic {
        values: vec![child(7)],
    };
    buf.ubf_write(&value, true).unwrap();
    assert_eq!(buf.ubf_read::<Generic<Child>>().unwrap(), value);
    let scalar = GenericScalar { value: 42_i64 };
    buf.ubf_write(&scalar, true).unwrap();
    assert_eq!(buf.ubf_read::<GenericScalar<i64>>().unwrap(), scalar);
}

#[test]
fn standalone_view_roundtrips_and_clears_optional_and_repeated_values() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut view = ctx
        .tpalloc_view("SERDE_VIEW", ctx.bvsizeof("SERDE_VIEW").unwrap())
        .unwrap();
    let mut value = position(42);
    value.values = vec![1, 2, 3, 4, 0];
    view.view_write(&value).unwrap();
    assert_eq!(view.view_read::<Position>().unwrap(), value);
    value.values = vec![0];
    value.note = None;
    value.blob = UbfCarray(vec![]);
    view.view_write(&value).unwrap();
    assert_eq!(view.view_read::<Position>().unwrap(), value);
    assert_eq!(view.bvoccur("values").unwrap().0, 1);
    assert!(view.bvnull("note", 0).unwrap());
    value.values.clear();
    view.view_write(&value).unwrap();
    assert_eq!(view.view_read::<Position>().unwrap(), value);
}

#[test]
fn deep_clone_preserves_pointer_children_after_original_is_dropped() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut original = ctx.tpalloc_ubf(128).unwrap();
    let value = mixed();
    original.ubf_write(&value, true).unwrap();
    let cloned = original.deep_clone().unwrap();
    drop(original);
    assert_eq!(cloned.ubf_read::<Mixed>().unwrap(), value);
}

#[test]
fn adhoc_mapping_deep_copies_instead_of_sharing_pointer_targets() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut native = ctx.tpalloc_ubf(128).unwrap();
    let value = mixed();
    native.ubf_write(&value, true).unwrap();
    let adhoc = UbfAdhoc(native);
    let mut outer = ctx.tpalloc_ubf(128).unwrap();
    adhoc
        .ubf_write_field(&mut outer, f::T_UBF_FLD, 0, true)
        .unwrap();
    drop(adhoc);
    let decoded: Mixed = endurox_rs::ubf_read_nested(&outer, f::T_UBF_FLD, 0).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn fixed_arrays_and_offsets_preserve_other_occurrences() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(4096).unwrap();
    for i in 0..6 {
        buf.bchg(f::T_LONG_FLD, i, 99_i64, false).unwrap();
    }
    [1_i64, 2]
        .ubf_write_field(&mut buf, f::T_LONG_FLD, 2, false)
        .unwrap();
    assert_eq!(buf.bget_long(f::T_LONG_FLD, 1).unwrap(), 99);
    assert_eq!(buf.bget_long(f::T_LONG_FLD, 4).unwrap(), 99);
    assert_eq!(
        <[i64; 2]>::ubf_read_field(&buf, f::T_LONG_FLD, 2).unwrap(),
        [1, 2]
    );
    let children = [child(1), child(2)];
    UbfMappedSerialize::<PointerUbf>::ubf_write_mapped(
        &children,
        &mut buf,
        f::T_PTR_FLD,
        1,
        128,
        true,
    )
    .unwrap();
    assert_eq!(
        <[Child; 2] as UbfMappedDeserialize<PointerUbf>>::ubf_read_mapped(&buf, f::T_PTR_FLD, 1)
            .unwrap(),
        children
    );
}

#[test]
fn mismatched_field_kinds_and_pointer_target_types_are_rejected() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(4096).unwrap();
    assert!(UbfMappedSerialize::<EmbeddedUbf>::ubf_write_mapped(
        &child(1),
        &mut buf,
        f::T_LONG_FLD,
        0,
        128,
        true
    )
    .is_err());
    buf.bchg(
        f::T_PTR_FLD,
        0,
        ctx.tpalloc_carray(b"wrong type").unwrap(),
        true,
    )
    .unwrap();
    assert!(endurox_rs::ubf_read_ptr::<Child>(&buf, f::T_PTR_FLD, 0).is_err());
    assert!(UbfMappedSerialize::<PointerUbf>::ubf_write_mapped(
        &Vec::<Child>::new(),
        &mut buf,
        f::T_UBF_FLD,
        0,
        128,
        true
    )
    .is_err());
    let mut wrong_view = ctx
        .tpalloc_view("TESTVIEW1", ctx.bvsizeof("TESTVIEW1").unwrap())
        .unwrap();
    assert!(wrong_view.view_write(&position(1)).is_err());
}

#[test]
fn occurrence_overflow_is_rejected_before_writing() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(1024).unwrap();
    assert!(UbfMappedSerialize::<PointerUbf>::ubf_write_mapped(
        &vec![child(1)],
        &mut buf,
        f::T_PTR_FLD,
        i32::MAX,
        128,
        true
    )
    .is_err());
    assert_eq!(ctx.boccur(&buf, f::T_PTR_FLD).unwrap(), 0);
    assert!(UbfMappedSerialize::<EmbeddedUbf>::ubf_write_mapped(
        &child(1),
        &mut buf,
        f::T_UBF_FLD,
        -1,
        128,
        true
    )
    .is_err());
}

#[test]
fn ambiguous_nested_collections_are_rejected() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(1024).unwrap();
    assert!(UbfMappedSerialize::<EmbeddedUbf>::ubf_write_mapped(
        &vec![Some(child(1))],
        &mut buf,
        f::T_UBF_FLD,
        0,
        128,
        true
    )
    .is_err());
    assert!(UbfMappedSerialize::<EmbeddedUbf>::ubf_write_mapped(
        &vec![vec![child(1)]],
        &mut buf,
        f::T_UBF_FLD,
        0,
        128,
        true
    )
    .is_err());
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct Defaults {
    #[ubf(field = f::T_UBF_FLD, nested, default)]
    child: DefaultChild,
}
#[derive(Debug, Default, PartialEq, UbfSerialize, UbfDeserialize)]
struct DefaultChild {
    #[ubf(field = f::T_LONG_FLD)]
    value: i64,
}
#[test]
fn default_applies_only_to_an_absent_outer_field() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(1024).unwrap();
    assert_eq!(buf.ubf_read::<Defaults>().unwrap().child.value, 0);
    buf.bchg(f::T_UBF_FLD, 0, ctx.tpalloc_ubf(1024).unwrap(), true)
        .unwrap();
    let error = buf.ubf_read::<Defaults>().unwrap_err();
    assert_eq!(error.code, endurox_rs::UbfError::BNOTPRES);
    assert!(error.message.contains("Defaults.child"));
    assert!(error.message.contains("DefaultChild.value"));
}

#[derive(UbfSerialize)]
struct Borrowed<'ctx> {
    #[ubf(field = f::T_STRING_FLD)]
    text: &'ctx str,
    #[ubf(field = f::T_UBF_FLD, nested)]
    child: &'ctx Child,
}
#[test]
fn serialization_accepts_borrowed_fields_and_ctx_named_lifetimes() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(128).unwrap();
    let source = child(4);
    buf.ubf_write(
        &Borrowed {
            text: "borrowed",
            child: &source,
        },
        true,
    )
    .unwrap();
    assert_eq!(buf.bget_string(f::T_STRING_FLD, 0).unwrap(), "borrowed");
    assert_eq!(
        endurox_rs::ubf_read_nested::<Child>(&buf, f::T_UBF_FLD, 0).unwrap(),
        source
    );
}

#[test]
fn integer_mappings_reject_overflow_and_support_unsigned_and_bool() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(1024).unwrap();
    65535_u16
        .ubf_write_field(&mut buf, f::T_LONG_FLD, 0, false)
        .unwrap();
    assert_eq!(u16::ubf_read_field(&buf, f::T_LONG_FLD, 0).unwrap(), 65535);
    assert!(i16::ubf_read_field(&buf, f::T_LONG_FLD, 0).is_err());
    assert!(65535_u16
        .ubf_write_field(&mut buf, f::T_SHORT_FLD, 0, false)
        .is_err());
    assert!(u64::MAX
        .ubf_write_field(&mut buf, f::T_LONG_FLD, 0, false)
        .is_err());
    true.ubf_write_field(&mut buf, f::T_SHORT_FLD, 0, false)
        .unwrap();
    assert!(bool::ubf_read_field(&buf, f::T_SHORT_FLD, 0).unwrap());
    buf.bchg(f::T_SHORT_FLD, 0, 2_i16, false).unwrap();
    assert!(bool::ubf_read_field(&buf, f::T_SHORT_FLD, 0).is_err());
    assert!(Vec::<i64>::ubf_read_field(&buf, f::T_LONG_FLD, -1).is_err());
    assert!(vec![1_i64, 2]
        .ubf_write_field(&mut buf, f::T_LONG_FLD, i32::MAX, false)
        .is_err());
}

#[derive(Debug, PartialEq, ViewSerialize, ViewDeserialize)]
#[view(name = "SERDE_VIEW")]
struct ViewOffsets {
    #[view(field = "values", occ = 1)]
    pair: [i64; 2],
    #[view(field = "name")]
    label: String,
}
#[test]
fn view_field_names_and_occurrence_offsets_are_mapped() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut view = ctx
        .tpalloc_view("SERDE_VIEW", ctx.bvsizeof("SERDE_VIEW").unwrap())
        .unwrap();
    view.bvchg("values", 0, 99_i64).unwrap();
    let value = ViewOffsets {
        pair: [1, 2],
        label: "renamed".into(),
    };
    view.view_write(&value).unwrap();
    assert_eq!(view.view_read::<ViewOffsets>().unwrap(), value);
    assert_eq!(view.bvget_i64("values", 0, 0).unwrap(), 99);
}

#[test]
fn serialization_failure_keeps_the_previous_pointer_target_alive() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(1024).unwrap();
    let original = child(1);
    endurox_rs::ubf_write_ptr(&mut buf, f::T_PTR_FLD, 0, &original, 1024, true).unwrap();
    let oversized = Child {
        id: 2,
        text: "x".repeat(8192),
    };
    assert!(endurox_rs::ubf_write_ptr(&mut buf, f::T_PTR_FLD, 0, &oversized, 1024, false).is_err());
    assert_eq!(
        endurox_rs::ubf_read_ptr::<Child>(&buf, f::T_PTR_FLD, 0).unwrap(),
        original
    );
}

#[test]
fn deep_clone_copies_null_pointer_placeholders_at_nonzero_offsets() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(1024).unwrap();
    endurox_rs::ubf_write_ptr(&mut buf, f::T_PTR_FLD, 2, &child(3), 1024, true).unwrap();
    let copy = buf.deep_clone().unwrap();
    drop(buf);
    assert_eq!(ctx.boccur(&copy, f::T_PTR_FLD).unwrap(), 3);
    assert!(copy.bget_ptr(f::T_PTR_FLD, 0).is_err());
    assert_eq!(
        endurox_rs::ubf_read_ptr::<Child>(&copy, f::T_PTR_FLD, 2).unwrap(),
        child(3)
    );
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct NullableOccurrences {
    #[ubf(field = f::T_PTR_FLD, ptr)]
    pointers: Vec<Option<Child>>,
    #[ubf(field = f::T_VIEW_FLD, view)]
    views: Vec<Option<Position>>,
}
#[test]
fn nullable_pointer_and_view_occurrences_keep_holes_and_trailing_nulls() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(1024).unwrap();
    let value = NullableOccurrences {
        pointers: vec![None, Some(child(1)), None],
        views: vec![Some(position(2)), None],
    };
    buf.ubf_write(&value, true).unwrap();
    assert_eq!(ctx.boccur(&buf, f::T_PTR_FLD).unwrap(), 3);
    assert_eq!(ctx.boccur(&buf, f::T_VIEW_FLD).unwrap(), 2);
    assert_eq!(buf.ubf_read::<NullableOccurrences>().unwrap(), value);
    let copy = buf.deep_clone().unwrap();
    drop(buf);
    assert_eq!(copy.ubf_read::<NullableOccurrences>().unwrap(), value);
}

#[test]
fn removed_pointer_occurrences_are_actually_freed() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(4096).unwrap();
    UbfMappedSerialize::<PointerUbf>::ubf_write_mapped(
        &vec![child(1), child(2)],
        &mut buf,
        f::T_PTR_FLD,
        0,
        1024,
        true,
    )
    .unwrap();
    // Extend only the phantom borrow for a native registry query. The context
    // remains alive, and tptypes checks registration without reading payload
    // memory. Never call as_bytes/field getters through this probe after free.
    let removed: endurox_rs::BorrowedBuffer<'static, 'static> =
        unsafe { std::mem::transmute(buf.bget_ptr(f::T_PTR_FLD, 1).unwrap()) };
    UbfMappedSerialize::<PointerUbf>::ubf_write_mapped(
        &vec![child(3)],
        &mut buf,
        f::T_PTR_FLD,
        0,
        1024,
        true,
    )
    .unwrap();
    assert!(
        removed.tptypes().is_err(),
        "truncated pointer target leaked"
    );

    // Inline-embedding a UBF that owns pointer targets is refused; such
    // sub-structures must be stored behind BFLD_PTR instead. The rejected value
    // drops, freeing the child it owned.
    let mut embedded = ctx.tpalloc_ubf(1024).unwrap();
    endurox_rs::ubf_write_ptr(&mut embedded, f::T_PTR_FLD, 0, &child(7), 1024, true).unwrap();
    let error = buf.bchg(f::T_UBF_FLD, 0, embedded, true).unwrap_err();
    assert_eq!(error.code, endurox_rs::UbfError::BEINVAL);
}

#[test]
fn shared_native_pointer_graph_requires_clone_before_destructive_edits() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(4096).unwrap();
    let target = ctx.tpalloc_ubf(1024).unwrap().into_inner();
    // Model an incoming C message with two references to one allocation. Both
    // wrappers are immediately consumed into that same parent, whose native
    // free cascade deduplicates the addresses. Rust's normal APIs cannot create
    // this shape, but the binding must safely handle native messages that do.
    let alias = unsafe { std::ptr::read(&target) };
    buf.bchg(f::T_PTR_FLD, 0, target, false).unwrap();
    buf.bchg(f::T_PTR_FLD, 1, alias, false).unwrap();
    assert!(buf.bdel_owned(f::T_PTR_FLD, 0).is_err());
    assert!(buf.bextract_ptr(f::T_PTR_FLD, 0).is_err());
    assert!(buf.bget_ptr_ubf(f::T_PTR_FLD, 0).is_ok());
    assert!(buf.bget_ptr_ubf(f::T_PTR_FLD, 1).is_ok());
    let mut clone = buf.deep_clone().unwrap();
    clone.bdel_owned(f::T_PTR_FLD, 0).unwrap();
    assert!(clone.bget_ptr_ubf(f::T_PTR_FLD, 0).is_ok());
}

#[test]
fn view_option_rejects_some_equal_to_the_null_sentinel() {
    use endurox_rs::ViewFieldSerialize;
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut view = ctx
        .tpalloc_view("SERDE_VIEW", ctx.bvsizeof("SERDE_VIEW").unwrap())
        .unwrap();
    assert!(Some(String::new())
        .view_write_field(&mut view, "note", 0)
        .is_err());
}

#[test]
fn excessive_pointer_nesting_returns_an_error_and_resets_read_state() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buffer = ctx.tpalloc_ubf(256).unwrap();
    buffer.bchg(f::T_LONG_FLD, 0, 0i64, false).unwrap();
    for id in 1..132i64 {
        let mut parent = ctx.tpalloc_ubf(256).unwrap();
        parent.bchg(f::T_LONG_FLD, 0, id, false).unwrap();
        parent
            .bchg(f::T_PTR_FLD, 0, buffer.into_inner(), false)
            .unwrap();
        buffer = parent;
    }
    let error = buffer.ubf_read::<Tree>().unwrap_err();
    assert!(error.message.contains("excessively deep"));
    // RAII removes every traversal entry even when a nested mapper fails.
    let mut shallow = ctx.tpalloc_ubf(256).unwrap();
    shallow.ubf_write(&child(42), false).unwrap();
    assert_eq!(shallow.ubf_read::<Child>().unwrap(), child(42));
}

// -------------------------------------------------------------------------
// Flat "group" mapping: accounts feed from grouped occurrences of flat root
// fields (no BFLD_UBF wrapper around the whole array), and a group element can
// still redirect one of its own fields into a per-element sub-buffer.
// -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
struct AcctDetail {
    #[ubf(field = f::T_STRING_4_FLD)]
    memo: String,
}

#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
#[ubf(group)]
struct GroupAccount {
    #[ubf(field = f::T_STRING_2_FLD)]
    number: String,
    #[ubf(field = f::T_LONG_2_FLD)]
    balance_cents: i64,
    // Each account keeps its own sub-buffer at its occurrence column.
    #[ubf(field = f::T_UBF_FLD, nested)]
    detail: AcctDetail,
}
fn group_account(n: i64) -> GroupAccount {
    GroupAccount {
        number: format!("ACC-{n}"),
        balance_cents: n * 1000,
        detail: AcctDetail {
            memo: format!("memo-{n}"),
        },
    }
}

#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
struct GroupCustomer {
    #[ubf(field = f::T_LONG_FLD)]
    id: i64,
    #[ubf(field = f::T_STRING_FLD)]
    name: String,
    #[ubf(group)]
    accounts: Vec<GroupAccount>,
}

#[test]
fn group_accounts_feed_from_flat_root_occurrences() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(256).unwrap();
    let mut value = GroupCustomer {
        id: 42,
        name: "Ada".into(),
        accounts: vec![group_account(1), group_account(2), group_account(3)],
    };
    buf.ubf_write(&value, true).unwrap();

    // One occurrence column per account, all in the single flat root buffer.
    assert_eq!(ctx.boccur(&buf, f::T_STRING_2_FLD).unwrap(), 3);
    assert_eq!(ctx.boccur(&buf, f::T_LONG_2_FLD).unwrap(), 3);
    assert_eq!(ctx.boccur(&buf, f::T_UBF_FLD).unwrap(), 3);
    assert_eq!(buf.bget_string(f::T_STRING_2_FLD, 1).unwrap(), "ACC-2");
    assert_eq!(buf.bget_long(f::T_LONG_2_FLD, 2).unwrap(), 3000);
    assert_eq!(buf.ubf_read::<GroupCustomer>().unwrap(), value);

    // A shorter vector truncates every group field's tail and frees sub-buffers.
    value.accounts.truncate(1);
    buf.ubf_write(&value, true).unwrap();
    assert_eq!(ctx.boccur(&buf, f::T_STRING_2_FLD).unwrap(), 1);
    assert_eq!(ctx.boccur(&buf, f::T_LONG_2_FLD).unwrap(), 1);
    assert_eq!(ctx.boccur(&buf, f::T_UBF_FLD).unwrap(), 1);
    assert_eq!(buf.ubf_read::<GroupCustomer>().unwrap(), value);
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct SingleGroupHolder {
    #[ubf(field = f::T_LONG_FLD)]
    id: i64,
    #[ubf(group)]
    account: GroupAccount,
}
#[test]
fn group_single_struct_feeds_from_the_base_occurrence() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(256).unwrap();
    let value = SingleGroupHolder {
        id: 7,
        account: group_account(9),
    };
    buf.ubf_write(&value, true).unwrap();
    assert_eq!(buf.bget_string(f::T_STRING_2_FLD, 0).unwrap(), "ACC-9");
    assert_eq!(buf.bget_long(f::T_LONG_FLD, 0).unwrap(), 7);
    assert_eq!(buf.ubf_read::<SingleGroupHolder>().unwrap(), value);
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
#[ubf(group)]
struct OffsetGroup {
    #[ubf(field = f::T_LONG_FLD, occ = 1)]
    id: i64,
    #[ubf(field = f::T_STRING_FLD, occ = 2)]
    text: String,
    #[ubf(field = f::T_PTR_FLD, ptr, occ = 3)]
    child: Option<Child>,
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct OffsetGroups {
    #[ubf(group, occ = 2)]
    rows: Vec<OffsetGroup>,
}

#[test]
fn group_offsets_preserve_prefixes_and_trim_each_members_tail() {
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(256).unwrap();
    buf.bchg(f::T_LONG_FLD, 0, 777i64, true).unwrap();
    buf.bchg(f::T_STRING_FLD, 0, "prefix", true).unwrap();
    let mut value = OffsetGroups {
        rows: vec![
            OffsetGroup {
                id: 10,
                text: "first".into(),
                child: None,
            },
            OffsetGroup {
                id: 20,
                text: "second".into(),
                child: Some(child(20)),
            },
            OffsetGroup {
                id: 30,
                text: "third".into(),
                child: None,
            },
        ],
    };
    buf.ubf_write(&value, true).unwrap();
    assert_eq!(buf.bget_long(f::T_LONG_FLD, 5).unwrap(), 30);
    assert_eq!(buf.ubf_read::<OffsetGroups>().unwrap(), value);
    value.rows.truncate(1);
    buf.ubf_write(&value, true).unwrap();
    assert_eq!(buf.bget_long(f::T_LONG_FLD, 3).unwrap(), 10);
    assert!(!ctx.bpres(&buf, f::T_LONG_FLD, 4));
    assert!(!ctx.bpres(&buf, f::T_STRING_FLD, 5));
    assert!(!ctx.bpres(&buf, f::T_PTR_FLD, 6));
    assert_eq!(buf.ubf_read::<OffsetGroups>().unwrap(), value);
    value.rows.clear();
    buf.ubf_write(&value, true).unwrap();
    assert_eq!(buf.ubf_read::<OffsetGroups>().unwrap(), value);
    assert_eq!(buf.bget_long(f::T_LONG_FLD, 0).unwrap(), 777);
    assert_eq!(buf.bget_string(f::T_STRING_FLD, 0).unwrap(), "prefix");
}

#[derive(Debug, UbfSerialize, UbfDeserialize)]
#[ubf(group)]
struct InvalidGroup<T> {
    #[ubf(field = f::T_LONG_FLD)]
    id: i64,
    #[ubf(field = f::T_STRING_FLD)]
    repeated: T,
}

#[test]
fn group_repeated_members_are_rejected_before_modifying_any_field() {
    use endurox_rs::{UbfGroupDeserialize, UbfGroupFieldSerialize, UbfGroupSerialize};
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(256).unwrap();
    buf.bchg(f::T_LONG_FLD, 0, 99i64, true).unwrap();
    let value = InvalidGroup {
        id: 1,
        repeated: vec!["a".to_string(), "b".into()],
    };
    assert!(value.ubf_write_at(&mut buf, 0, true).is_err());
    assert_eq!(buf.bget_long(f::T_LONG_FLD, 0).unwrap(), 99);
    assert!(InvalidGroup::<Vec<String>>::ubf_read_at(&buf, 0).is_err());
    let array = InvalidGroup {
        id: 1,
        repeated: ["a".to_string(), "b".into()],
    };
    assert!(array.ubf_write_at(&mut buf, 0, true).is_err());
    let optional = InvalidGroup {
        id: 1,
        repeated: None::<String>,
    };
    assert!(optional.ubf_write_at(&mut buf, 0, true).is_err());
    let empty: Vec<InvalidGroup<Vec<String>>> = vec![];
    assert!(empty.ubf_group_write_field(&mut buf, 0, true).is_err());
    assert_eq!(buf.bget_long(f::T_LONG_FLD, 0).unwrap(), 99);
}

#[test]
fn group_occurrence_overflow_and_negative_offsets_return_errors() {
    use endurox_rs::{UbfGroupDeserialize, UbfGroupFieldSerialize, UbfGroupSerialize};
    #[derive(UbfSerialize, UbfDeserialize)]
    #[ubf(group)]
    struct NegativeOffset {
        #[ubf(field = f::T_LONG_FLD, occ = -1)]
        id: i64,
    }
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(256).unwrap();
    let value = OffsetGroup {
        id: 1,
        text: "value".into(),
        child: None,
    };
    assert!(value.ubf_write_at(&mut buf, i32::MAX, true).is_err());
    assert!(OffsetGroup::ubf_read_at(&buf, i32::MAX).is_err());
    assert!(OffsetGroup::ubf_clear_from(&mut buf, i32::MAX).is_err());
    assert!(NegativeOffset { id: 1 }
        .ubf_write_at(&mut buf, 2, true)
        .is_err());
    assert!(NegativeOffset::ubf_read_at(&buf, 2).is_err());
    let rows = vec![value];
    // The final member's offset overflows: no earlier member may be written.
    assert!(rows
        .ubf_group_write_field(&mut buf, i32::MAX - 2, true)
        .is_err());
    assert!(!ctx.bpres(&buf, f::T_LONG_FLD, 0));
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
#[ubf(group)]
struct NullableGroup {
    #[ubf(field = f::T_PTR_FLD, ptr)]
    child: Option<Child>,
}

#[test]
fn nullable_group_anchor_keeps_null_rows_and_fixed_array_neighbors() {
    use endurox_rs::{UbfGroupFieldDeserialize, UbfGroupFieldSerialize};
    let _guard = super::endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut buf = ctx.tpalloc_ubf(256).unwrap();
    let values = vec![
        NullableGroup {
            child: Some(child(1)),
        },
        NullableGroup {
            child: Some(child(2)),
        },
        NullableGroup {
            child: Some(child(3)),
        },
    ];
    values.ubf_group_write_field(&mut buf, 0, true).unwrap();
    [NullableGroup { child: None }]
        .ubf_group_write_field(&mut buf, 1, true)
        .unwrap();
    let expected = vec![
        NullableGroup {
            child: Some(child(1)),
        },
        NullableGroup { child: None },
        NullableGroup {
            child: Some(child(3)),
        },
    ];
    assert_eq!(
        Vec::<NullableGroup>::ubf_group_read_field(&buf, 0).unwrap(),
        expected
    );
    let nulls = vec![NullableGroup { child: None }, NullableGroup { child: None }];
    nulls.ubf_group_write_field(&mut buf, 0, true).unwrap();
    assert_eq!(ctx.boccur(&buf, f::T_PTR_FLD).unwrap(), 2);
    assert_eq!(
        Vec::<NullableGroup>::ubf_group_read_field(&buf, 0).unwrap(),
        nulls
    );
}
