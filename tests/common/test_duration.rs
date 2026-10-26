//! Shared integration stress duration in seconds.
use std::time::Duration;

/// Read NDRX_TEST_DURATATION, defaulting to ten seconds when absent or empty.
/// Invalid values fail the test instead of silently skipping its body.
pub fn test_duration() -> Duration {
    let value = std::env::var("NDRX_TEST_DURATATION").unwrap_or_default();
    let value = if value.is_empty() { "10" } else { &value };
    assert!(
        value.bytes().all(|digit| digit.is_ascii_digit()),
        "NDRX_TEST_DURATATION must be a positive integer in seconds"
    );
    let seconds = value
        .parse::<i32>()
        .expect("NDRX_TEST_DURATATION must be between 1 and 2147483647 seconds");
    assert!(seconds > 0, "NDRX_TEST_DURATATION must be positive");
    Duration::from_secs(seconds as u64)
}
