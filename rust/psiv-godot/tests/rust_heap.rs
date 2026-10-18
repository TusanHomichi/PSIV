//! The counting allocator (`src/rust_heap.rs`), in a process of its own.
//!
//! The counters are process-wide, so a test sharing a process with others
//! races their allocations (it failed about one run in ten inside the crate's
//! unit-test binary). This file compiles the allocator module on its own,
//! with one test, so nothing else allocates in its window.

#[path = "../src/rust_heap.rs"]
mod rust_heap;

use rust_heap::snapshot;

#[test]
fn a_deliberate_allocation_shows_in_live_bytes_and_leaves_with_it() {
    let before = snapshot();
    let block = vec![7u8; 4 << 20];
    let during = snapshot();
    assert!(
        during[0] >= before[0] + (4 << 20),
        "live bytes {before:?} -> {during:?}"
    );
    assert!(during[2] > before[2]);
    drop(std::hint::black_box(block));
    let after = snapshot();
    assert!(after[0] + (3 << 20) <= during[0], "{during:?} -> {after:?}");
}
