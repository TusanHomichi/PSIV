//! The extension's own heap, counted (#92).
//!
//! Every Rust allocation in the extension (psiv-godot and the crates it links:
//! psiv-runtime, psiv-core, psiv-data, psiv-sound) goes through this global
//! allocator, which forwards to the system allocator and keeps three relaxed
//! atomic counters: live bytes, live allocations and allocations ever made.
//! RSS cannot stand in for live bytes, because the system allocator keeps
//! freed pages; the memory check (`tools/native_rss.py`) judges these
//! counters instead.
//!
//! Not counted: Godot's own allocator (reported by Godot's static-memory
//! monitor), and C code calling `malloc` directly. psiv-sound's Nuked OPN2
//! bridge makes one `calloc` for its chip state when the sound machine is
//! built (`psiv_ym_new`, once per process) and none while it runs.
//!
//! Always on, so the measured binary is the shipped one.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

struct Counting;

static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

fn added(bytes: usize) {
    LIVE_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    LIVE_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
}

fn removed(bytes: usize) {
    LIVE_BYTES.fetch_sub(bytes as u64, Ordering::Relaxed);
    LIVE_ALLOCATIONS.fetch_sub(1, Ordering::Relaxed);
}

// SAFETY: every method forwards to `System` with the caller's arguments and
// only adds counter updates, which allocate nothing.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            added(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            added(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged.
        unsafe { System.dealloc(pointer, layout) };
        removed(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            if new_size >= layout.size() {
                LIVE_BYTES.fetch_add((new_size - layout.size()) as u64, Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub((layout.size() - new_size) as u64, Ordering::Relaxed);
            }
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The counters now: live bytes, live allocations, allocations ever made
/// (a reallocation counts as one).
pub(crate) fn snapshot() -> [u64; 3] {
    [
        LIVE_BYTES.load(Ordering::Relaxed),
        LIVE_ALLOCATIONS.load(Ordering::Relaxed),
        ALLOCATIONS.load(Ordering::Relaxed),
    ]
}
