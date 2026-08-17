//! Field boot data and the flag view consumed by presentation widgets.

use std::path::Path;

use psiv_core::{CharId, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::GameData;
use psiv_runtime::Runtime;

use crate::input::requested_save_slot;

/// Fallback spawn when the pack predates game-start extraction.
pub(crate) const FALLBACK_SPAWN_MAP: u16 = 0x010;
pub(crate) const FALLBACK_SPAWN_CELL: (u16, u16) = (31, 8);

pub(crate) fn collect_event_flags(rt: &Runtime) -> Vec<bool> {
    (0..512u16)
        .map(|id| rt.game().is_set(psiv_core::Flag::event(id)))
        .collect()
}

/// Title-screen bypasses used by save/debug fix loops. These are deliberately
/// checked before the title node is built: a screenshot or a battle/camp/shop
/// selector must still reach the surface it was written to inspect.
pub(crate) fn title_bypassed() -> bool {
    let title_shot = std::env::var("PSIV_DEBUG_TITLE_SHOT").is_ok_and(|value| value == "1");
    requested_save_slot().is_some()
        || std::env::var_os("PSIV_DEBUG_BATTLE").is_some()
        || std::env::args().any(|argument| argument.starts_with("--psiv-debug-battle="))
        || std::env::var("PSIV_DEBUG_CAMP").is_ok_and(|value| value == "1")
        || std::env::var_os("PSIV_DEBUG_SHOP").is_some()
        || std::env::var_os("PSIV_DEBUG_EVENT").is_some()
        || (std::env::var_os("PSIV_DEBUG_SHOT").is_some() && !title_shot)
}

/// Builds the two deterministic scene fixtures used by the shell's oracle
/// harness. These are in-memory retail saves, not files and not a product
/// boot path: GameStart needs its scripted second actor present, while
/// MeetingRika's map is the BioPlant B4 entry used by the runtime tests.
pub(crate) fn debug_scene_runtime(
    data: GameData,
    event: u16,
    step_frames: StepFrames,
) -> Option<Result<Runtime, String>> {
    let (map, char_x, char_y, party) = match event {
        0x009F => (
            0x0013,
            48 * 16,
            19 * 16,
            [Some(CharId(0)), Some(CharId(1)), None, None, None],
        ),
        0x8007 => (
            0x00AC,
            16,
            16,
            [
                Some(CharId(0)),
                Some(CharId(1)),
                Some(CharId(2)),
                Some(CharId(3)),
                None,
            ],
        ),
        _ => return None,
    };
    let mut game = GameState::new();
    game.set_party(party);
    Some(
        Runtime::from_save(
            data,
            RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: map,
                    char_x,
                    char_y,
                },
            },
            step_frames,
        )
        .map_err(|error| error.to_string()),
    )
}

/// Valid retail-shaped slots visible to the title menu. Loading is the same
/// checksum/slot validation the continue path uses; no save bytes are
/// written, repaired, or reserialized here.
pub(crate) fn available_save_slots(data: &GameData, directory: &Path) -> [bool; 3] {
    std::array::from_fn(|slot| {
        Runtime::load_slot(data.clone(), directory, slot, StepFrames::default()).is_ok()
    })
}
