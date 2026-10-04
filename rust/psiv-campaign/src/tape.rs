//! Compatibility path for campaign callers. The tape codec and FNV hash have
//! one owner in `psiv-runtime`, shared with the native replay feed.

pub use psiv_runtime::tape::{Tape, TapeError, TapeStart, fnv1a64};
