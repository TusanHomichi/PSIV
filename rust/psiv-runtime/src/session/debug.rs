//! Test and certification fixtures: sessions that start from a hand-built
//! state, in the runtime, so no shell ever needs a mutable runtime for one.
//!
//! Every fixture here builds from `Runtime::new_game` — the retail initializer
//! — and then applies explicit flag, party, position and object edits, because
//! a fixture that invents its own banks rots the moment the opening's triggers
//! or the new-game state change (#44). The selectors that *choose* a fixture
//! stay in the shell (`PSIV_DEBUG_CAMP`, `PSIV_DEBUG_EVENT`, the battle
//! selectors); the state they need is here.
//!
//! Nothing in this module is a product path: it is not reachable from
//! [`Session::frame`], and the tests that use it are named fixtures throughout.

use psiv_core::{CharId, Direction, Flag, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::GameData;

use crate::events::BattleTimeline;
use crate::{Runtime, Session};

use super::battle::BattleMode;
use super::{BattleFrame, BattleStart, Mode};

/// The `PSIV_DEBUG_EVENT=0x8007` MeetingRika fixture: the BioPlant B4 entry
/// with a full party, an in-memory retail save and no file behind it.
///
/// `None` for an event this family has no state for; the shell reports that as
/// "no fixture for this event" rather than starting a scene on a wrong map.
#[must_use]
pub fn scene_fixture(
    data: GameData,
    event: u16,
    step_frames: StepFrames,
) -> Option<Result<Session, String>> {
    // The ship's destination menu (`PSIV_DEBUG_EVENT=0x800D`): the Mota
    // Spaceport at the foot of the gangway, `EventFlag_AlysFound` set so the
    // list is the post-Zio one (`ps4.asm:133728-133735`), a Chaz and Alys party.
    let mut flags: &[u16] = &[];
    let (map, char_x, char_y, party) = match event {
        0x800D => {
            flags = &[0x08];
            (
                0x00BF,
                30 * 16,
                20 * 16,
                [Some(CharId(0)), Some(CharId(1)), None, None, None],
            )
        }
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
    for &flag in flags {
        if let Err(error) = game.set(Flag::event(flag)) {
            return Some(Err(format!("scene fixture flag {flag:#x}: {error}")));
        }
    }
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
        .map(Session::new)
        .map_err(|error| error.to_string()),
    )
}

/// The `PSIV_DEBUG_CAMP=1` fixture: tape 22's camp frame, Chaz alone at map
/// `$13`, field position `($2F0,$140)`, with 500 MST.
///
/// The state is the retail new-game banks plus the two flags the opening earns
/// before control (`EventFlag 7` from Event_GameStart,
/// `EventFlag_PiataChazControl` `$15` from Event_PiataChazAlone) and tape 22's
/// live object positions. The camera is the receipt's own settled position:
/// retail's camera at `($258,$E8)`, 16 pixels below the fixture's
/// player-centred default, reflecting the walk history the fixture does not
/// replay.
pub fn camp_fixture(data: GameData, step_frames: StepFrames) -> Result<Session, String> {
    let mut game = post_opening_game(&data, step_frames)?;
    game.set_party([Some(CharId(0)), None, None, None, None]);
    game.set_money(500);
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
    let mut runtime = Runtime::from_save(
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
    .map_err(|error| error.to_string())?;
    for (index, &(x, y, facing)) in OBJECTS.iter().enumerate() {
        runtime
            .set_npc_pixel_position(index, x, y)
            .map_err(|error| format!("camp object {index} position failed: {error}"))?;
        runtime.face_npc(index, facing);
    }
    // Camp opens with the field suspended, so the 30-frame debug lead-in
    // cannot consume another wander step before the receipt-backed state is
    // drawn.
    runtime.set_field_suspended(true);
    runtime.set_camera(0x258, 0xE8);
    Ok(Session::new(runtime))
}

/// The retail new-game banks plus the two flags the opening earns before
/// control: the state tape 22's camp frame is in.
///
/// A blank [`GameState::new`] has `$15` clear, so the map-entry trigger 124
/// replays PiataChazAlone on load and pre-empts the fixture (#44).
fn post_opening_game(data: &GameData, step_frames: StepFrames) -> Result<GameState, String> {
    let mut game = Runtime::new_game(data.clone(), step_frames)
        .map_err(|error| error.to_string())?
        .game()
        .clone();
    for id in [7, 21] {
        game.set(Flag::event(id))
            .map_err(|error| format!("post-opening flag {id}: {error}"))?;
    }
    Ok(game)
}

/// The debug selectors that drive a session from outside its own frame.
///
/// Each one is what a `PSIV_DEBUG_*` variable does at its tick: the shell
/// reads the variable, and the state it asks for is built here so no shell
/// needs a mutable runtime for it.
impl Session {
    /// Debug-selector family: starts `formation` as an ordinary encounter
    /// battle from outside the field frame, which is what
    /// `PSIV_DEBUG_BATTLE=<formation>` and `PSIV_DEBUG_VEHICLE_BATTLE` do.
    ///
    /// A real battle starts, so the capture that follows plays the same rounds
    /// a player would; nothing about it is a fixture.
    pub fn debug_battle(&mut self, formation: u16) -> BattleFrame {
        match self.start_encounter(formation) {
            Ok(timeline) => {
                let mut mode =
                    BattleMode::begin(&self.runtime, timeline, BattleStart::Encounter(formation));
                let frame = mode.start_frame(&self.runtime);
                self.mode = Mode::Battle(Box::new(mode));
                frame
            }
            Err(error) => BattleFrame {
                view: None,
                started: None,
                fault: Some(format!("debug battle {formation:#05x} refused: {error}")),
            },
        }
    }

    /// Debug-selector family: the newly-exact probe of `PSIV_DEBUG_BATTLE=0x89`.
    ///
    /// A real `formation` battle starts and its opening timeline is replaced by
    /// `timeline`, so a decoded enemy attack can be watched on the live screen
    /// without a command menu. The battle is real, which means the shared RNG
    /// advances here where the old fixture's did not; no certified capture
    /// covers this selector.
    pub fn debug_battle_probe(&mut self, formation: u16, timeline: BattleTimeline) -> BattleFrame {
        let party = self.runtime.battle_party();
        if party.is_empty() {
            return BattleFrame {
                view: None,
                started: None,
                fault: Some(format!(
                    "debug probe {formation:#05x} refused: the battle party is empty"
                )),
            };
        }
        match self.runtime.start_battle(formation, party) {
            Ok(_) => {
                let mut mode =
                    BattleMode::begin(&self.runtime, timeline, BattleStart::Encounter(formation));
                let frame = mode.start_frame(&self.runtime);
                self.mode = Mode::Battle(Box::new(mode));
                frame
            }
            Err(error) => BattleFrame {
                view: None,
                started: None,
                fault: Some(format!("debug probe {formation:#05x} refused: {error}")),
            },
        }
    }

    /// Debug-selector family: starts a transcribed scene from outside the field
    /// frame, which is what `PSIV_DEBUG_EVENT=<event>` does.
    ///
    /// `false` means the pack has no scene for that event, or the runtime
    /// refused to start one (a map-entry trigger already holds the story).
    pub fn debug_start_event(&mut self, event: u16) -> bool {
        self.runtime.start_event(event)
    }

    /// Debug-selector family: mounts vehicle `index`, which is what
    /// `PSIV_DEBUG_VEHICLE_INDEX` and `PSIV_DEBUG_VEHICLE` do before the first
    /// frame.
    ///
    /// # Errors
    ///
    /// Whatever [`Runtime::set_vehicle_index`] rejects: an unknown index, or a
    /// mount the current map cannot carry.
    pub fn debug_mount_vehicle(&mut self, index: u16) -> Result<(), crate::BridgeError> {
        self.runtime.set_vehicle_index(index)
    }
}
