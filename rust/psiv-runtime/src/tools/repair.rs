//! The legacy-save repair, as one operation.
//!
//! `docs/camp/PROGRESSION.md` records why this exists: saves written before
//! level-up learning was implemented are missing the techniques, skills and
//! skill maxima the level tables already contained. Ordinary load never
//! rewrites a roster, so the repair is explicit — a copied source slot in, a
//! repaired output slot out, and the source file untouched.
//!
//! This is not a game path: it loads a slot, repairs it, writes it back and
//! hands over a report, so no caller ever holds the runtime.

use std::path::{Path, PathBuf};

use psiv_core::StepFrames;
use psiv_data::{BattleFiles, GameData};

use crate::Runtime;
use crate::progression::ProgressionRepair;

/// What a repair changed, and where the repaired slot went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressionRepairReport {
    /// One entry per character whose abilities or maxima were restored.
    pub repairs: Vec<ProgressionRepair>,
    /// The slot file that was written.
    pub saved: PathBuf,
}

/// Repairs the legacy save in `source`'s `slot`, writes the result to
/// `output`'s `slot`, and reports what changed.
///
/// The repair is idempotent: the second pass over the same runtime must find
/// nothing left to do, and it runs before anything is written, so a save the
/// repair cannot finish leaves no output behind.
///
/// # Errors
///
/// The pack or its battle files cannot be read, `source` holds no such slot or
/// an invalid one, the repair itself is refused (no battle data, or work in
/// flight), or the output slot cannot be written.
pub fn repair_legacy_progression(
    pack: &Path,
    source: &Path,
    output: &Path,
    slot: usize,
) -> Result<ProgressionRepairReport, String> {
    let data = GameData::load(pack).map_err(|error| format!("pack: {error}"))?;
    let files = BattleFiles::load(pack).map_err(|error| format!("battle files: {error}"))?;
    let mut runtime = Runtime::load_slot(data, source, slot, StepFrames::default())
        .map_err(|error| format!("{}: {error}", source.display()))?;
    runtime
        .enable_battles(&files)
        .map_err(|error| format!("battles: {error}"))?;
    let repairs = runtime
        .repair_legacy_progression()
        .map_err(|error| error.to_string())?;
    let again = runtime
        .repair_legacy_progression()
        .map_err(|error| error.to_string())?;
    if !again.is_empty() {
        return Err(format!(
            "the repair is not idempotent: a second pass changed {} more character(s)",
            again.len()
        ));
    }
    let saved = runtime
        .save_slot(output, slot)
        .map_err(|error| format!("{}: {error}", output.display()))?;
    Ok(ProgressionRepairReport { repairs, saved })
}
