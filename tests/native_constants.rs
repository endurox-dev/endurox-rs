use endurox_rs::{
    AtmiCtx, AtmiError, NstdError, TpQCtl, UbfError, UbfFieldType, BBADFLDID, BBADFLDOCC,
    BFIRSTFLDID, BFLD_CARRAY, BFLD_CHAR, BFLD_DOUBLE, BFLD_FLOAT, BFLD_LONG, BFLD_PTR, BFLD_SHORT,
    BFLD_STRING, BFLD_UBF, BFLD_VIEW, BNOTPRES, NEPRECOND, NEUNAVAILABLE, QMEINVAL, QMENOMSG,
    TPEINVAL, TPNOABORT, TPNOCACHELOOK, TPQCORRID, TPQGETBYCORRID,
};

#[test]
fn native_field_type_constants_work_with_field_id_apis() {
    let ctx = AtmiCtx::new().unwrap();
    for (code, kind) in [
        (BFLD_SHORT, UbfFieldType::Short),
        (BFLD_LONG, UbfFieldType::Long),
        (BFLD_CHAR, UbfFieldType::Char),
        (BFLD_FLOAT, UbfFieldType::Float),
        (BFLD_DOUBLE, UbfFieldType::Double),
        (BFLD_STRING, UbfFieldType::String),
        (BFLD_CARRAY, UbfFieldType::Carray),
        (BFLD_PTR, UbfFieldType::Ptr),
        (BFLD_UBF, UbfFieldType::Ubf),
        (BFLD_VIEW, UbfFieldType::View),
    ] {
        let field = ctx.bmkfldid(code, 1).unwrap();
        assert_ne!(field, BBADFLDID);
        assert_eq!(ctx.bfldtype(field).unwrap(), kind);
    }
}

#[test]
fn public_codes_preserve_error_construction_and_negative_sentinels() {
    let atmi = AtmiError::new(AtmiError::TPEINVAL, "invalid argument");
    let ubf = UbfError::new(UbfError::BNOTPRES, "missing field");
    let nstd = NstdError::new(NstdError::NEPRECOND, "condition changed");
    assert_eq!(atmi.code, TPEINVAL);
    assert_eq!(ubf.code, BNOTPRES);
    assert_eq!(nstd.code, NEPRECOND);
    assert_eq!(nstd.message, "condition changed");
    assert_eq!(NstdError::NEUNAVAILABLE, NEUNAVAILABLE);

    let path_terminator: i32 = BBADFLDOCC;
    let queue_diagnostics: [i64; 2] = [QMEINVAL, QMENOMSG];
    assert_eq!(path_terminator, -1);
    assert!(queue_diagnostics.into_iter().all(|code| code < 0));
    assert_eq!(BFIRSTFLDID, BBADFLDID);
}

#[test]
fn flag_exports_fit_public_control_apis() {
    let call_flags: i64 = TPNOABORT | TPNOCACHELOOK;
    assert_ne!(call_flags, 0);
    let mut control = TpQCtl::default();
    control.set_flags(TPQCORRID).add_flags(TPQGETBYCORRID);
    assert_eq!(control.flags(), TPQCORRID | TPQGETBYCORRID);
    control.clear_flags(TPQGETBYCORRID);
    assert_eq!(control.flags(), TPQCORRID);
}
