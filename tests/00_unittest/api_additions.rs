use super::endurox_test_env;
use endurox_rs::{
    dlm::*, ubf_fields as f, AtmiCtx, AtmiError, TpDlmCtl, TpEvCtl, UbfError, UbfFieldRef,
    UbfValue, TPEVPERSIST, TPEVSERVICE,
};

#[test]
fn event_controls_validate_and_preserve_fields() {
    let mut ctl = TpEvCtl::default();
    ctl.set_name1("RS_EVENT")
        .unwrap()
        .set_flags(TPEVSERVICE | TPEVPERSIST);
    ctl.set_name2("reserved").unwrap();
    assert_eq!(ctl.name1(), "RS_EVENT");
    assert_eq!(ctl.name2(), "reserved");
    assert_eq!(ctl.flags(), TPEVSERVICE | TPEVPERSIST);
    assert!(ctl.set_name1("bad\0name").is_err());
    assert!(ctl.set_name1(&"x".repeat(1024)).is_err());
    assert_eq!(ctl.name1(), "RS_EVENT");
}

#[test]
fn ubf_search_and_last_occurrences_preserve_types_and_values() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut ubf = ctx.tpalloc_ubf(ctx.bneeded(12, 256).unwrap()).unwrap();
    for text in ["hello.first", "hello.second", "other"] {
        ubf.badd(f::T_STRING_FLD, text, false).unwrap();
    }
    let exact = UbfValue::String("hello.second".into());
    assert_eq!(ubf.bfindocc(f::T_STRING_FLD, &exact, false).unwrap(), 1);
    let pattern = UbfValue::String("hello\\..*".into());
    assert_eq!(ubf.bfindocc(f::T_STRING_FLD, &pattern, true).unwrap(), 0);
    assert_eq!(
        ubf.bfindocc(f::T_STRING_FLD, &pattern, false)
            .unwrap_err()
            .code,
        UbfError::BNOTPRES
    );
    assert!(ubf
        .bfindocc(f::T_STRING_FLD, &UbfValue::String("[".into()), true)
        .is_err());
    assert_eq!(
        ubf.bfindocc(f::T_STRING_FLD, &UbfValue::Long(1), false)
            .unwrap_err()
            .code,
        UbfError::BTYPERR
    );
    let blob = vec![1, 0, 2, 0, 255];
    ubf.badd(f::T_CARRAY_FLD, blob.clone(), false).unwrap();
    ubf.badd(f::T_CARRAY_FLD, Vec::<u8>::new(), false).unwrap();
    assert_eq!(
        ubf.bfindocc(f::T_CARRAY_FLD, &UbfValue::Carray(blob.clone()), false)
            .unwrap(),
        0
    );
    assert_eq!(
        ubf.bfindocc(f::T_CARRAY_FLD, &UbfValue::Carray(vec![]), false)
            .unwrap(),
        1
    );
    let (occ, last) = ubf.bfindlast(f::T_STRING_FLD).unwrap();
    assert_eq!(occ, 2);
    assert!(matches!(last, UbfFieldRef::String(s) if s.to_bytes() == b"other"));
    let (_, last) = ubf.bgetlast(f::T_STRING_FLD).unwrap();
    ubf.bchg(f::T_STRING_FLD, 2, "changed", true).unwrap();
    assert!(matches!(last, UbfValue::String(s) if s == "other"));
    assert!(
        matches!(ubf.bfindlast(f::T_CARRAY_FLD).unwrap(), (1, UbfFieldRef::Carray(b)) if b.is_empty())
    );
    assert!(ubf.bfindlast(f::T_LONG_FLD).is_err());
    assert!(ubf.bgetlast(f::T_LONG_FLD).is_err());
    for (field, value) in [
        (f::T_SHORT_FLD, UbfValue::Short(-123)),
        (f::T_LONG_FLD, UbfValue::Long(123456)),
        (f::T_CHAR_FLD, UbfValue::Char(65)),
        (f::T_FLOAT_FLD, UbfValue::Float(1.25)),
        (f::T_DOUBLE_FLD, UbfValue::Double(2.5)),
    ] {
        ubf.badd(field, value, false).unwrap();
        let (occ, copy) = ubf.bgetlast(field).unwrap();
        assert_eq!(occ, 0);
        assert_eq!(ubf.bfindocc(field, &copy, false).unwrap(), 0);
        assert_eq!(ubf.bfindlast(field).unwrap().0, 0);
    }
    for (count, size) in [(0, 1), (1, 0), (usize::MAX, 1), (1, usize::MAX)] {
        assert_eq!(
            ctx.bneeded(count, size).unwrap_err().code,
            UbfError::BEINVAL
        );
    }
}

#[test]
fn ubf_last_complex_values_borrow_or_copy_independently() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    let mut parent = ctx.tpalloc_ubf(4096).unwrap();
    let mut child = ctx.tpalloc_ubf(512).unwrap();
    child.bchg(f::T_STRING_FLD, 0, "child", false).unwrap();
    parent
        .badd(f::T_UBF_FLD, UbfValue::Ubf(child), false)
        .unwrap();
    parent
        .badd(f::T_PTR_FLD, ctx.tpalloc_carray(b"pointer").unwrap(), false)
        .unwrap();
    let mut view = ctx.tpalloc_view("TESTVIEW1", 0).unwrap();
    view.bvchg("tlong", 0, 42_i64).unwrap();
    parent.bchg_view(f::T_VIEW_FLD, 0, &view, false).unwrap();
    view.bvchg("tlong", 0, 99_i64).unwrap();
    parent.bchg_view(f::T_VIEW_2_FLD, 0, &view, false).unwrap();
    let search = UbfValue::View(view);
    assert_eq!(parent.bfindocc(f::T_VIEW_2_FLD, &search, false).unwrap(), 0);
    assert_eq!(
        parent
            .bfindocc(f::T_VIEW_FLD, &search, false)
            .unwrap_err()
            .code,
        UbfError::BNOTPRES
    );
    let (_, UbfFieldRef::View(Some(borrowed))) = parent.bfindlast(f::T_VIEW_FLD).unwrap() else {
        panic!("VIEW expected")
    };
    let (_, UbfFieldRef::View(Some(other))) = parent.bfindlast(f::T_VIEW_2_FLD).unwrap() else {
        panic!("VIEW expected")
    };
    assert_eq!(borrowed.bvget_i64("tlong", 0, 0).unwrap(), 42);
    assert_eq!(other.bvget_i64("tlong", 0, 0).unwrap(), 99);
    let (_, UbfFieldRef::Ubf(borrowed)) = parent.bfindlast(f::T_UBF_FLD).unwrap() else {
        panic!("UBF expected")
    };
    assert_eq!(borrowed.bget_string(f::T_STRING_FLD, 0).unwrap(), "child");
    let (_, UbfFieldRef::Ptr(Some(borrowed))) = parent.bfindlast(f::T_PTR_FLD).unwrap() else {
        panic!("PTR expected")
    };
    assert_eq!(borrowed.tptypes().unwrap().type_name, "CARRAY");
    drop(borrowed);
    let (_, UbfValue::Ubf(child_copy)) = parent.bgetlast(f::T_UBF_FLD).unwrap() else {
        panic!("UBF expected")
    };
    let (_, UbfValue::Ptr(pointer_copy)) = parent.bgetlast(f::T_PTR_FLD).unwrap() else {
        panic!("PTR expected")
    };
    let (_, UbfValue::View(view_copy)) = parent.bgetlast(f::T_VIEW_FLD).unwrap() else {
        panic!("VIEW expected")
    };
    drop(parent);
    assert_eq!(child_copy.bget_string(f::T_STRING_FLD, 0).unwrap(), "child");
    assert_eq!(pointer_copy.tptypes().unwrap().type_name, "CARRAY");
    assert_eq!(view_copy.bvget_i64("tlong", 0, 0).unwrap(), 42);
}

#[test]
fn request_logging_routes_all_facilities_and_preserves_contexts() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    ctx.tpinit().unwrap();
    ctx.tplogclosereqfile();
    assert_eq!(ctx.tploggetreqfile(), None);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/00_unittest/log/request-api.log");
    let name = path.to_str().unwrap();
    let mut ubf = ctx.tpalloc_ubf(4096).unwrap();
    ctx.tplogsetreqfile(Some(&mut ubf), Some(name), None)
        .unwrap();
    assert_eq!(ctx.tploggetbufreqfile(&ubf).unwrap(), name);
    assert_eq!(ctx.tploggetreqfile().as_deref(), Some(name));
    endurox_rs::tp_always!(ctx, "REQUEST-TP-MARKER");
    endurox_rs::ndrx_always!(ctx, "REQUEST-NDRX-MARKER");
    endurox_rs::ubf_always!(ctx, "REQUEST-UBF-MARKER");
    #[cfg(feature = "ctx-send")]
    {
        let other = AtmiCtx::new().unwrap();
        assert_eq!(other.tploggetreqfile(), None);
        endurox_rs::tp_always!(other, "OTHER-CONTEXT-MARKER");
        assert_eq!(ctx.tploggetreqfile().as_deref(), Some(name));
    }
    ctx.tplogclosereqfile();
    assert_eq!(ctx.tploggetreqfile(), None);
    let text = std::fs::read_to_string(&path).unwrap();
    for marker in [
        "REQUEST-TP-MARKER",
        "REQUEST-NDRX-MARKER",
        "REQUEST-UBF-MARKER",
    ] {
        assert!(text.contains(marker), "missing {marker}");
    }
    assert!(!text.contains("OTHER-CONTEXT-MARKER"));
    ctx.tplogsetreqfile(Some(&mut ubf), None, None).unwrap();
    ctx.tplogdelbufreqfile(&mut ubf).unwrap();
    assert_eq!(
        ctx.tploggetbufreqfile(&ubf).unwrap_err().code,
        AtmiError::TPENOENT
    );
    assert_eq!(ctx.tploggetreqfile().as_deref(), Some(name));
    ctx.tplogclosereqfile();
    ctx.tplogsetreqfile_direct(name).unwrap();
    assert_eq!(ctx.tploggetreqfile().as_deref(), Some(name));
    ctx.tplogclosethread();
    ctx.tplogclosereqfile();
    assert!(ctx.tplogsetreqfile(None, None, None).is_err());
    assert!(ctx.tplogsetreqfile_direct("bad\0name").is_err());
    assert!(ctx.tplogsetreqfile_direct(&"a".repeat(32768)).is_err());
}

#[test]
fn dlm_preparation_grows_nested_requests_and_preserves_identity_rules() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().unwrap();
    ctx.tpinit().unwrap();
    let mut ctl = TpDlmCtl::default();
    ctl.set_dlmspace("RUST")
        .unwrap()
        .set_wait_time(2500)
        .unwrap();
    let mut request = ctx.tpalloc_ubf(128).unwrap();
    let mut command = ctx.tpalloc_ubf(128).unwrap();
    command
        .bchg(EX_DLM_CMD, 0, NDRX_DLM_CMD_LOCK, true)
        .unwrap();
    command.bchg(EX_DLM_KEY, 0, "rust-key", true).unwrap();
    let spare = ctx.bunused(&command).unwrap();
    command
        .bchg(f::T_CARRAY_FLD, 0, vec![0_u8; spare - 32], false)
        .unwrap();
    request
        .bchg(EX_DLM_CMDENTRY, 0, command.into_inner(), true)
        .unwrap();
    let before = request
        .bget_ptr(EX_DLM_CMDENTRY, 0)
        .unwrap()
        .tptypes()
        .unwrap()
        .size;
    ctx.tpdlmmkcall(&mut request, &mut ctl).unwrap();
    assert!(!ctl.svcname().is_empty());
    assert!(ctl.svcname().contains("RUST"));
    assert_eq!(request.bget_long(EX_DLM_WAIT_TIME, 0).unwrap(), 2500);
    assert_eq!(request.len(), 0);
    let nested = request.bget_ptr_ubf(EX_DLM_CMDENTRY, 0).unwrap();
    let cid = nested.bget_string(EX_DLM_CID, 0).unwrap();
    assert!(!cid.is_empty());
    assert_eq!(nested.bget_string(EX_DLM_SEQNO, 0).unwrap(), "-1");
    assert!(
        request
            .bget_ptr(EX_DLM_CMDENTRY, 0)
            .unwrap()
            .tptypes()
            .unwrap()
            .size
            > before
    );
    let first = request.bget_string(EX_DLM_REQCID, 0).unwrap();
    ctx.tpdlmmkcall(&mut request, &mut ctl).unwrap();
    let second = request.bget_string(EX_DLM_REQCID, 0).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        request
            .bget_ptr_ubf(EX_DLM_CMDENTRY, 0)
            .unwrap()
            .bget_string(EX_DLM_CID, 0)
            .unwrap(),
        cid
    );
    request
        .bchg(EX_DLM_OP, 0, NDRX_DLM_OP_CANCEL, true)
        .unwrap();
    ctx.tpdlmmkcall(&mut request, &mut ctl).unwrap();
    assert_eq!(request.bget_string(EX_DLM_REQCID, 0).unwrap(), second);
    let timing = ctx.tpdlmtoutget(2500).unwrap();
    assert_eq!(timing.total_millis, 2500);
    assert!(timing.attempt_millis <= timing.total_millis);
    assert!(timing.block_seconds >= 1);
    assert!(ctx.tpdlmattemptsget().unwrap() >= 1);
    assert!(ctl.set_dlmspace(&"x".repeat(256)).is_err());
    assert!(ctl.set_wait_time(-1).is_err());
    assert!(ctl.set_flags(1).is_err());
    let mut invalid = ctx.tpalloc_ubf(512).unwrap();
    invalid
        .bchg(
            EX_DLM_CMDENTRY,
            0,
            ctx.tpalloc_carray(b"not UBF").unwrap(),
            false,
        )
        .unwrap();
    assert_eq!(
        ctx.tpdlmmkcall(&mut invalid, &mut ctl).unwrap_err().code,
        AtmiError::TPEINVAL
    );
}
