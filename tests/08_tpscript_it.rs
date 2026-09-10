#[path = "common/endurox_domain_lock.rs"]
mod endurox_domain_lock;

#[test]
fn native_scripting_and_rust_callbacks() {
    let _guard = endurox_domain_lock::lock_endurox_domain();
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/08_tpscript");
    let output = std::process::Command::new("bash")
        .arg(format!("{directory}/run.sh"))
        .current_dir(directory)
        .output()
        .expect("could not start scripting integration fixture");
    assert!(
        output.status.success(),
        "scripting tests failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
}
