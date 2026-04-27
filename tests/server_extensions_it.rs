use std::process::Command;
use std::sync::{Mutex, OnceLock};

#[test]
fn xatmi_server_extensions_poller_and_before_poll_callbacks() {
    let _guard = match integration_test_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let test_dir = manifest_dir
        .join("tests")
        .join("xatmi_server_extensions_api");
    let run_sh = test_dir.join("run.sh");

    assert!(
        run_sh.exists(),
        "missing integration script: {}",
        run_sh.display()
    );

    let output = Command::new("bash")
        .arg(&run_sh)
        .current_dir(&test_dir)
        .output()
        .expect("failed to execute run.sh");

    if !output.status.success() {
        panic!(
            "run.sh failed with status={}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn integration_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
