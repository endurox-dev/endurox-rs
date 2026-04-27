use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use endurox_rs::AtmiCtx;
use endurox_rs::TypedBuffer;
use endurox_rs::TypedUbf;
use endurox_rs::UbfValue;

#[test]
fn atmictx_init_integration() {
    let _guard = endurox_test_env();

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
    let _guard = endurox_test_env();

    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    // generic typed buffer
    let tbuf: TypedBuffer<'_> = ctx.tpalloc("UBF", "", 0).expect("tpalloc failed");

    // "inherit" by casting to TypedUbf
    let mut ubf: TypedUbf<'_> = TypedUbf::from_typed(tbuf);

    assert!(ubf.bsizeof().expect("Bsizeof failed") > 0);

    //ctx.tpterm().expect("tpterm failed");
    ctx.tpinit().expect("Second init shall go OK");
}

#[test]
fn tpalloc_ubf() {
    let _guard = endurox_test_env();

    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    endurox_rs::ndrx_error!(ctx, ">>>>> About to alloc UBF...");
    let mut buf = ctx.tpalloc_ubf(1025).expect("Shall Alloc buffer OK");

    buf.bchg(1, 0, UbfValue::Long(5), false)
        .expect("Bchg failed");

    endurox_rs::ndrx_error!(ctx, ">>>>> About to free UBF...");
    drop(buf);
    drop(ctx);
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
#export LANG=en_UK.UTF-8
unset NDRX_DEBUG_CONF
export NDRX_DEBUG_STR="file=$NDRX_RS_UNIT_TEST_DIR/log/unittest.log ndrx=5"
# print the env to load later by rust bin
env
"#,
            )
            .env("NDRX_RS_UNIT_TEST_DIR", &test_dir)
            .env("NDRX_RS_UNIT_UBF_FILE", &ubf_file)
            .output()
            .expect("failed to run xadmin provision for atmictx tests");

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
