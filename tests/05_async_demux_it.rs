#[path = "common/endurox_domain_lock.rs"]
mod endurox_domain_lock;
use endurox_domain_lock::lock_endurox_domain;

use std::process::Command;

/// Real native queue pressure without requiring a pollable reply backend.
#[test]
fn async_send_retries_preserve_priority_and_other_tasks_settings() {
    let _guard = lock_endurox_domain();
    let test_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/05_async_demux");
    let output = Command::new("bash")
        .arg(format!("{test_dir}/run.sh"))
        .arg("--send-only")
        .current_dir(test_dir)
        .output()
        .expect("failed to execute send-only run.sh");
    assert!(
        output.status.success(),
        "native async send regression failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// End-to-end proof that concurrent async calls on one context both complete.
///
/// Only meaningful on a pollable Enduro/X build: elsewhere `into_tokio()`
/// returns `TPEINVAL` and there is no reply fd to demultiplex, so the scenario
/// cannot be constructed at all.
#[cfg(endurox_pollable)]
#[test]
fn async_calls_are_demultiplexed_by_call_descriptor() {
    let _guard = lock_endurox_domain();

    let test_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/05_async_demux");
    let run_sh = format!("{test_dir}/run.sh");

    let output = Command::new("bash")
        .arg(&run_sh)
        .current_dir(test_dir)
        .output()
        .expect("failed to execute run.sh");

    if !output.status.success() {
        panic!(
            "async demux integration test failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[cfg(not(endurox_pollable))]
#[test]
fn async_calls_are_demultiplexed_by_call_descriptor() {
    eprintln!(
        "skipped: this Enduro/X build has no pollable reply queue \
         (needs EX_USE_EPOLL, or EX_USE_KQUEUE on FreeBSD)"
    );
}
