//! Replaying a tape: the same session, the same pads, and the digest they end
//! on.
//!
//! A replay uses the [`Driver`] a run does, so a fault that would have halted
//! the run halts the replay at the same frame. A SAVE request in the tape is
//! answered `Ok` without touching the disk: the original run wrote the file,
//! and the answer is all the game sees.

use std::path::Path;

use psiv_runtime::Pad;

use crate::digest::Digest;
use crate::driver::Driver;
use crate::start::{SetupError, StartPoint, open_session};
use crate::tape::{Tape, TapeStart, fnv1a64};

/// What a replay ended on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayResult {
    /// Frames played.
    pub frames: u64,
    /// The final-state digest.
    pub digest: Digest,
    /// Frames the driver would have halted a run on, with the reason: a run
    /// that halted ends its tape on the frame that faulted, and replaying it
    /// reproduces that frame.
    pub faults: Vec<(usize, String)>,
}

/// Replays `tape` over the pack in `pack`.
///
/// `from_save` is the slot file a tape that starts from a save began on; its
/// hash must match the tape's.
///
/// # Errors
///
/// [`SetupError`] when the pack or save cannot be opened, or the save is not the
/// one the tape names.
pub fn replay(
    pack: &Path,
    tape: &Tape,
    from_save: Option<&Path>,
) -> Result<ReplayResult, SetupError> {
    let start = match (&tape.start, from_save) {
        (TapeStart::NewGame, None) => StartPoint::NewGame,
        (TapeStart::NewGame, Some(_)) => {
            return Err(SetupError(
                "the tape starts at power-on; --from-save does not apply".into(),
            ));
        }
        (TapeStart::Save { .. }, None) => {
            return Err(SetupError(
                "the tape starts from a save; pass it with --from-save".into(),
            ));
        }
        (TapeStart::Save { hash }, Some(path)) => {
            let bytes = std::fs::read(path)
                .map_err(|e| SetupError(format!("cannot read save {}: {e}", path.display())))?;
            if fnv1a64(&bytes) != *hash {
                return Err(SetupError(format!(
                    "{} is not the save the tape starts from (hash {:016x}, tape wants {hash:016x})",
                    path.display(),
                    fnv1a64(&bytes)
                )));
            }
            StartPoint::Save(path.to_path_buf())
        }
    };
    let (session, _) = open_session(pack, &start)?;
    let mut driver = Driver::new(session, None);
    let mut faults = Vec::new();
    for (frame, byte) in tape.pads.iter().enumerate() {
        if let Err(halt) = driver.tick(Pad::from_bits(*byte)) {
            faults.push((frame, halt.to_string()));
        }
    }
    Ok(ReplayResult {
        frames: driver.frames(),
        digest: Digest::of(driver.runtime(), driver.frames()),
        faults,
    })
}
