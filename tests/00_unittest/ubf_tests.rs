use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use endurox_rs::{ubf_fields, AtmiCtx, TypedBuffer, TypedUbf, UbfFieldType, UbfValue};

#[test]
fn ubf_change_and_get_scalar_fields() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(ubf_fields::T_SHORT_FLD, 0, UbfValue::Short(123), false)
        .expect("short Bchg failed");
    ubf.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(456789), false)
        .expect("long Bchg failed");
    ubf.bchg(ubf_fields::T_CHAR_FLD, 0, UbfValue::Char(b'Z' as i8), false)
        .expect("char Bchg failed");
    ubf.bchg(ubf_fields::T_FLOAT_FLD, 0, UbfValue::Float(12.5), false)
        .expect("float Bchg failed");
    ubf.bchg(ubf_fields::T_DOUBLE_FLD, 0, UbfValue::Double(123.75), false)
        .expect("double Bchg failed");
    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("hello-ubf".to_string()),
        false,
    )
    .expect("string Bchg failed");

    assert_eq!(ubf.bget_short(ubf_fields::T_SHORT_FLD, 0).unwrap(), 123);
    assert_eq!(ubf.bget_long(ubf_fields::T_LONG_FLD, 0).unwrap(), 456789);
    assert_eq!(
        ubf.bget_char(ubf_fields::T_CHAR_FLD, 0).unwrap(),
        b'Z' as i8
    );
    assert!((ubf.bget_float(ubf_fields::T_FLOAT_FLD, 0).unwrap() - 12.5).abs() < f32::EPSILON);
    assert!((ubf.bget_double(ubf_fields::T_DOUBLE_FLD, 0).unwrap() - 123.75).abs() < f64::EPSILON);
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "hello-ubf"
    );
}

#[test]
fn ubf_change_and_get_carray_field() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");
    let bytes = vec![0, 1, 2, 3, 254, 255];

    ubf.bchg(
        ubf_fields::T_CARRAY_FLD,
        0,
        UbfValue::Carray(bytes.clone()),
        false,
    )
    .expect("carray Bchg failed");

    assert_eq!(ubf.bget_bytes(ubf_fields::T_CARRAY_FLD, 0).unwrap(), bytes);
}

#[test]
fn ubf_change_and_get_embedded_ubf_field() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut outer = ctx.tpalloc_ubf(4096).expect("outer tpalloc_ubf failed");
    let mut inner = ctx.tpalloc_ubf(1024).expect("inner tpalloc_ubf failed");

    inner
        .bchg(
            ubf_fields::T_STRING_3_FLD,
            0,
            UbfValue::String("inside".to_string()),
            false,
        )
        .expect("inner string Bchg failed");
    outer
        .bchg(ubf_fields::T_UBF_FLD, 0, UbfValue::Ubf(inner), false)
        .expect("embedded UBF Bchg failed");

    let borrowed = outer.bget_ubf(ubf_fields::T_UBF_FLD, 0).unwrap();
    assert_eq!(
        borrowed.bget_string(ubf_fields::T_STRING_3_FLD, 0).unwrap(),
        "inside"
    );
}

#[test]
fn ubf_multiple_occurrences_are_indexed() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("first".to_string()),
        false,
    )
    .expect("first occurrence Bchg failed");
    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        1,
        UbfValue::String("second".to_string()),
        false,
    )
    .expect("second occurrence Bchg failed");

    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "first"
    );
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 1).unwrap(),
        "second"
    );
}

#[test]
fn ubf_missing_field_returns_error() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let ubf = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");

    let err = ubf
        .bget_string(ubf_fields::T_STRING_FLD, 0)
        .expect_err("missing field should fail");
    assert_ne!(err.code, 0);
}

#[test]
fn ubf_buffer_can_be_reallocated() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");
    let original_size = ubf.bsizeof().expect("Bsizeof failed");

    ubf.tprealloc(original_size * 2).expect("tprealloc failed");
    let new_size = ubf.bsizeof().expect("Bsizeof after tprealloc failed");

    assert!(new_size >= original_size * 2);
}

#[test]
fn ubf_generic_buffer_can_be_cast_to_ubf() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let generic: TypedBuffer<'_> = ctx.tpalloc("UBF", "", 2048).expect("tpalloc failed");
    let mut ubf = TypedUbf::from_typed(generic);

    ubf.bchg(
        ubf_fields::T_STRING_2_FLD,
        0,
        UbfValue::String("cast-ok".to_string()),
        false,
    )
    .expect("Bchg on cast buffer failed");

    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_2_FLD, 0).unwrap(),
        "cast-ok"
    );
}

#[test]
fn atmictx_ubf_presence_count_length_and_delete_apis() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("first".to_string()),
        false,
    )
    .expect("first occurrence Bchg failed");
    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        1,
        UbfValue::String("second".to_string()),
        false,
    )
    .expect("second occurrence Bchg failed");
    ubf.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(99), false)
        .expect("long Bchg failed");

    assert!(ctx.bpres(&ubf, ubf_fields::T_STRING_FLD, 0));
    assert_eq!(ctx.boccur(&ubf, ubf_fields::T_STRING_FLD).unwrap(), 2);
    assert!(ctx.blen(&ubf, ubf_fields::T_STRING_FLD, 0).unwrap() >= "first".len());
    assert!(ctx.bnum(&ubf).unwrap() >= 3);
    assert!(ctx.bisubf(&ubf));
    assert!(ctx.bsizeof(&ubf).unwrap() > 0);
    assert!(ctx.bused(&ubf).unwrap() > 0);
    assert!(ctx.bunused(&ubf).unwrap() > 0);

    ctx.bdel(&mut ubf, ubf_fields::T_STRING_FLD, 0)
        .expect("Bdel failed");
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "second"
    );

    ubf.bchg(
        ubf_fields::T_STRING_2_FLD,
        0,
        UbfValue::String("delete-all-1".to_string()),
        false,
    )
    .expect("delete-all first occurrence Bchg failed");
    ubf.bchg(
        ubf_fields::T_STRING_2_FLD,
        1,
        UbfValue::String("delete-all-2".to_string()),
        false,
    )
    .expect("delete-all second occurrence Bchg failed");
    ctx.bdelall(&mut ubf, ubf_fields::T_STRING_2_FLD)
        .expect("Bdelall failed");
    assert!(!ctx.bpres(&ubf, ubf_fields::T_STRING_2_FLD, 0));

    let mut delete_list = [ubf_fields::T_LONG_FLD, 0];
    ctx.bdelete(&mut ubf, &mut delete_list)
        .expect("Bdelete failed");
    assert!(!ctx.bpres(&ubf, ubf_fields::T_LONG_FLD, 0));
}

#[test]
fn atmictx_ubf_copy_compare_project_and_subset_apis() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut src = ctx.tpalloc_ubf(4096).expect("source tpalloc_ubf failed");
    let mut dst = ctx.tpalloc_ubf(4096).expect("dest tpalloc_ubf failed");
    let mut projected = ctx.tpalloc_ubf(4096).expect("projected tpalloc_ubf failed");

    src.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("keep".to_string()),
        false,
    )
    .expect("source string Bchg failed");
    src.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(1234), false)
        .expect("source long Bchg failed");

    ctx.bcpy(&mut dst, &src).expect("Bcpy failed");
    assert!(ctx.bcmp(&src, &dst));
    assert!(ctx.bsubset(&src, &dst));

    let mut project_list = [ubf_fields::T_STRING_FLD, 0];
    ctx.bprojcpy(&mut projected, &src, &mut project_list)
        .expect("Bprojcpy failed");
    assert_eq!(
        projected.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "keep"
    );
    assert!(!ctx.bpres(&projected, ubf_fields::T_LONG_FLD, 0));

    let mut in_place_project_list = [ubf_fields::T_LONG_FLD, 0];
    ctx.bproj(&mut dst, &mut in_place_project_list)
        .expect("Bproj failed");
    assert!(!ctx.bpres(&dst, ubf_fields::T_STRING_FLD, 0));
    assert_eq!(dst.bget_long(ubf_fields::T_LONG_FLD, 0).unwrap(), 1234);
}

#[test]
fn atmictx_ubf_update_concat_join_and_index_apis() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut dst = ctx.tpalloc_ubf(4096).expect("dest tpalloc_ubf failed");
    let mut src = ctx.tpalloc_ubf(4096).expect("source tpalloc_ubf failed");

    dst.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("old".to_string()),
        false,
    )
    .expect("dest string Bchg failed");
    src.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("new".to_string()),
        false,
    )
    .expect("source string Bchg failed");
    src.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(77), false)
        .expect("source long Bchg failed");

    ctx.bupdate(&mut dst, &src).expect("Bupdate failed");
    assert_eq!(dst.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(), "new");
    assert_eq!(dst.bget_long(ubf_fields::T_LONG_FLD, 0).unwrap(), 77);

    ctx.bconcat(&mut dst, &src).expect("Bconcat failed");
    assert!(ctx.boccur(&dst, ubf_fields::T_STRING_FLD).unwrap() >= 2);

    let mut joined = ctx.tpalloc_ubf(4096).expect("joined tpalloc_ubf failed");
    joined
        .bchg(
            ubf_fields::T_STRING_FLD,
            0,
            UbfValue::String("join-target".to_string()),
            false,
        )
        .expect("joined seed Bchg failed");
    ctx.bjoin(&mut joined, &src).expect("Bjoin failed");
    assert_eq!(
        joined.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "new"
    );

    let mut outer_joined = ctx
        .tpalloc_ubf(4096)
        .expect("outer joined tpalloc_ubf failed");
    outer_joined
        .bchg(
            ubf_fields::T_STRING_2_FLD,
            0,
            UbfValue::String("kept".to_string()),
            false,
        )
        .expect("outer joined seed Bchg failed");
    ctx.bojoin(&mut outer_joined, &src).expect("Bojoin failed");
    assert_eq!(
        outer_joined
            .bget_string(ubf_fields::T_STRING_2_FLD, 0)
            .unwrap(),
        "kept"
    );

    ctx.bindex(&mut dst, 0).expect("Bindex failed");
    let _ = ctx.bidxused(&dst).expect("Bidxused failed");
    let _ = ctx.bunindex(&mut dst).expect("Bunindex failed");
}

#[test]
fn atmictx_ubf_field_id_helper_uses_defined_types() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    assert_eq!(
        ctx.bmkfldid_typed(UbfFieldType::String, 1061),
        ubf_fields::T_STRING_FLD
    );
    assert_eq!(
        ctx.bmkfldid_typed(UbfFieldType::Long, 1031),
        ubf_fields::T_LONG_FLD
    );
}

#[test]
fn atmictx_ubf_error_paths_set_ubf_error() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");

    let err = ctx
        .bdel(&mut ubf, ubf_fields::T_STRING_FLD, 0)
        .expect_err("Bdel on missing field should fail");
    assert_ne!(err.code, 0);

    let missing = ubf
        .bget_long(ubf_fields::T_LONG_FLD, 0)
        .expect_err("missing long should fail");
    assert_ne!(missing.code, 0);
}

fn endurox_test_env() -> MutexGuard<'static, ()> {
    let guard = match endurox_test_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    provision_endurox_env();
    guard
}

fn endurox_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn provision_endurox_env() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let test_dir = manifest_dir.join("tests").join("00_unittest");
        let ubf_file = manifest_dir.join("tests").join("ubftab").join("test.fd");

        let output = Command::new("bash")
            .arg("-lc")
            .arg(
                r#"
set -euo pipefail
cd "$NDRX_RS_UNIT_TEST_DIR"
if [ -f "$HOME/ndrx_home" ]; then
    . "$HOME/ndrx_home"
fi
rm -f conf/app.ini conf/settest1
mkdir -p log
find log -type f -exec rm -f {} +
xadmin provision -d -vaddubf="$NDRX_RS_UNIT_UBF_FILE" >/dev/null
. conf/settest1
unset NDRX_DEBUG_CONF
export NDRX_DEBUG_STR="file=$NDRX_RS_UNIT_TEST_DIR/log/ubf-tests.log ndrx=5"
env
"#,
            )
            .env("NDRX_RS_UNIT_TEST_DIR", &test_dir)
            .env("NDRX_RS_UNIT_UBF_FILE", &ubf_file)
            .output()
            .expect("failed to run xadmin provision for UBF tests");

        if !output.status.success() {
            panic!(
                "xadmin provision failed with status={}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some((key, value)) = line.split_once('=') {
                env::set_var(key, value);
            }
        }
    });
}
