#[path = "common/endurox_domain_lock.rs"]
mod endurox_domain_lock;
use endurox_domain_lock::lock_endurox_domain;

use std::process::Command;

#[test]
fn xatmi_server_client_tpcall_ubf_roundtrip() {
    run_xatmi_server_client_scenario("tpcall");
}

#[test]
fn event_request_logging_and_dlm_apis() {
    run_xatmi_server_client_scenario("api-additions");
    run_xatmi_server_client_scenario_with_feature("api-additions", "ctx-send");
}

#[test]
fn xatmi_server_client_tpacall_ubf_roundtrip() {
    run_xatmi_server_client_scenario("tpacall");
}

#[test]
fn xatmi_expression_callbacks_preserve_pending_reply_with_oapi() {
    run_xatmi_server_client_scenario_with_feature("tpacall", "ctx-send");
}

#[test]
fn xatmi_server_client_tpacall_getany_ubf_roundtrip() {
    run_xatmi_server_client_scenario("tpacall-getany");
}

#[test]
fn xatmi_server_client_tpforward_ubf_roundtrip() {
    run_xatmi_server_client_scenario("tpforward");
}

#[test]
fn xatmi_server_client_embedded_ubf_roundtrip() {
    run_xatmi_server_client_scenario("inner-ubf");
}

#[test]
fn xatmi_server_dispatches_on_multiple_worker_threads() {
    run_xatmi_server_client_scenario("dispatch-threads");
}

#[test]
fn xatmi_server_dispatches_on_multiple_worker_threads_with_oapi() {
    run_xatmi_server_client_scenario_with_feature("dispatch-threads", "ctx-send");
}

#[test]
fn xatmi_server_single_dispatch_thread_with_oapi() {
    run_xatmi_server_client_scenario_inner("tpcall", Some("ctx-send"), "single");
}

#[test]
fn xatmi_server_replies_with_buffer_other_than_request() {
    run_xatmi_server_client_scenario("reply-new");
    run_xatmi_server_client_scenario_with_feature("reply-new", "ctx-send");
    run_xatmi_server_client_scenario_inner("reply-new", None, "single");
}

fn run_xatmi_server_client_scenario(scenario: &str) {
    run_xatmi_server_client_scenario_inner(scenario, None, "multi");
}

fn run_xatmi_server_client_scenario_with_feature(scenario: &str, feature: &str) {
    run_xatmi_server_client_scenario_inner(scenario, Some(feature), "multi");
}

fn run_xatmi_server_client_scenario_inner(
    scenario: &str,
    feature: Option<&str>,
    dispatch_mode: &str,
) {
    let _guard = lock_endurox_domain();
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let test_dir = manifest_dir.join("tests").join("01_server_api");
    let run_sh = test_dir.join("run.sh");

    assert!(
        run_sh.exists(),
        "missing integration script: {}",
        run_sh.display()
    );

    let output = Command::new("bash")
        .arg(&run_sh)
        .arg(scenario)
        .arg(feature.unwrap_or(""))
        .arg(dispatch_mode)
        .current_dir(&test_dir)
        .output()
        .expect("failed to execute run.sh");

    if !output.status.success() {
        panic!(
            "run.sh {scenario} failed with status={}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn typed_replies_reject_carray_images_and_invalid_forward_replies_with_failure() {
    run_xatmi_server_client_scenario("reply-types");
    run_xatmi_server_client_scenario_with_feature("reply-types", "ctx-send");
}

#[test]
fn inplace_calls_preserve_ownership_types_and_error_replies() {
    run_xatmi_server_client_scenario("inplace");
    run_xatmi_server_client_scenario_with_feature("inplace", "ctx-send");
}
