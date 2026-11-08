//! Exercise the production RSS query independently of a running Godot engine.
//! This integration target is discovered by the ordinary Cargo test command
//! on each native platform, including release-template hosts.

#[path = "../src/resident_memory.rs"]
mod resident_memory;

#[test]
fn native_process_resident_memory_is_available() {
    let resident_kb = resident_memory::resident_memory_kb();
    assert!(
        resident_kb > 0,
        "the native resident-memory query failed: {resident_kb} KiB"
    );
}
