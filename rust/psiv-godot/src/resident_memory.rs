//! One process-memory observation for every native platform.
//!
//! `memory-stats` uses GetProcessMemoryInfo's WorkingSetSize on Windows,
//! task_info's resident_size on macOS, and procfs resident pages on Linux.
//! It returns bytes; the tape format reports KiB. Failure is explicit, never
//! a zero measurement that could conceal a missing platform implementation.

pub(crate) fn resident_memory_kb() -> i64 {
    memory_stats::memory_stats()
        .and_then(|stats| i64::try_from(stats.physical_mem / 1024).ok())
        .filter(|value| *value > 0)
        .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_process_resident_memory_is_available() {
        assert!(super::resident_memory_kb() > 0);
    }
}
