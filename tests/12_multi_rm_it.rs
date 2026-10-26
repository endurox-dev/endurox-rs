#[path = "common/endurox_domain_lock.rs"]
mod endurox_domain_lock;

#[test]
fn rm_server_and_client() {
    let _guard = endurox_domain_lock::lock_endurox_domain();
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/12_multi_rm");
    for feature in ["", "ctx-send"] {
        let output = std::process::Command::new("bash")
            .arg(format!("{directory}/run.sh"))
            .arg(feature)
            .current_dir(directory)
            .output()
            .expect("could not start rm integration fixture");
        assert!(
            output.status.success(),
            "12_multi_rm ({feature}) failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    }
}
