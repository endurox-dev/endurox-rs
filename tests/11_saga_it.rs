#[path = "common/endurox_domain_lock.rs"]
mod endurox_domain_lock;

#[test]
fn saga_server_and_client() {
    let _guard = endurox_domain_lock::lock_endurox_domain();
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/11_saga");
    for feature in ["", "ctx-send"] {
        let output = std::process::Command::new("bash")
            .arg(format!("{directory}/run.sh"))
            .arg(feature)
            .current_dir(directory)
            .output()
            .expect("could not start saga integration fixture");
        assert!(
            output.status.success(),
            "11_saga ({feature}) failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    }
}
