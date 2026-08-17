//! Field boot data and the flag view consumed by presentation widgets.

use std::path::Path;

use psiv_core::{CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
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
        || std::env::var_os("PSIV_DEBUG_VEHICLE").is_some()
        || std::env::var_os("PSIV_DEBUG_VEHICLE_BATTLE").is_some()
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
            // The oracle's tape-28 fixture: leader at pixel ($1F0,$1A0) —
            // the retail trigger requires leader Y exactly $1A0.
            0x1F0,
            0x1A0,
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

/// Reproduces tape 22's `camp_root_idle` receipt without making a save file:
/// Chaz alone at map `$13`, field position `($2F0,$140)`, with 500 MST. The
/// location is the runtime's player coordinate; the oracle camera settles at
/// `($258,$E8)` after the map renderer applies its viewport offset.
pub(crate) fn debug_camp_runtime(
    data: GameData,
    step_frames: StepFrames,
) -> Option<Result<Runtime, String>> {
    if !std::env::var("PSIV_DEBUG_CAMP").is_ok_and(|value| value == "1") {
        return None;
    }
    let mut game = GameState::new();
    game.set_party([Some(CharId(0)), None, None, None, None]);
    game.set_money(500);
    // Tape 22, mark `camp_root_idle`, frame 7675. These are live RAM object
    // positions, not map-spawn guesses. Camp opens with the field suspended,
    // so the 30-frame debug lead-in cannot consume another wander step before
    // the receipt-backed state is drawn.
    const OBJECTS: [(i32, i32, Direction); 8] = [
        (736, 226, Direction::Down),
        (743, 128, Direction::Left),
        (624, 128, Direction::Left),
        (352, 112, Direction::Down),
        (320, 256, Direction::Right),
        (271, 160, Direction::Left),
        (256, 112, Direction::Down),
        (592, 240, Direction::Down),
    ];
    Some(
        Runtime::from_save(
            data,
            RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0xFFFF,
                    map_index: 0x13,
                    char_x: 0x2F0,
                    char_y: 0x140,
                },
            },
            step_frames,
        )
        .map_err(|error| error.to_string())
        .and_then(|mut runtime| {
            for (index, &(x, y, facing)) in OBJECTS.iter().enumerate() {
                runtime
                    .set_npc_pixel_position(index, x, y)
                    .map_err(|error| format!("camp object {index} position failed: {error}"))?;
                runtime.face_npc(index, facing);
            }
            runtime.set_field_suspended(true);
            Ok(runtime)
        }),
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
