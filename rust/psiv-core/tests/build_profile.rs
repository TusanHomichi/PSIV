//! The test profile keeps the checks retail integer widths rely on.
//!
//! `rust/Cargo.toml`'s `[profile.test]` raises `opt-level` so the gate's
//! pack-loading suites run in minutes. Optimization must not turn off overflow
//! checks or debug assertions: a wrap that retail code never performs has to
//! panic in tests, not pass silently. These cases fail if either is disabled.

use std::hint::black_box;
use std::panic;

#[test]
fn integer_overflow_panics_in_test_builds() {
    let wrapped = panic::catch_unwind(|| black_box(u8::MAX) + black_box(1u8));
    assert!(
        wrapped.is_err(),
        "u8::MAX + 1 wrapped to {:?}: overflow-checks is off in the test profile",
        wrapped.ok()
    );
}

#[test]
fn debug_assertions_fire_in_test_builds() {
    let asserted = panic::catch_unwind(|| debug_assert!(black_box(false)));
    assert!(
        asserted.is_err(),
        "debug_assert!(false) passed: debug-assertions is off in the test profile"
    );
}
