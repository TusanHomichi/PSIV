//! Gyuna's conversation and Tyler's grave, driven with pads from hand-built
//! saves (lane S7).
//!
//! The route's `dezolis-gyuna` and `dezolis-tyler-grave` chapters play both from
//! New Game in the ignored whole-route test in `runner.rs`; these keep the two
//! seams they lean on cheap enough for the ordinary suite: the dialogue's stop
//! byte deciding `$81`, and the grave's twelve objects drifting off the stairs.
//! They need the rebuilt pack (`scene chunk atlas` for Tyler's chunk `$47`) and
//! skip with a message without a local one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_campaign::field::Settled;
use psiv_core::{Cell, CharId, Direction, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const RYUON_PUB: u16 = 0x14A;
const TYLER: u16 = 0x120;
const GYUNA_TOLD: u16 = 0x81;
const TYLER_GRAVE: u16 = 0x84;
const SNOWSTORM: u16 = 0x80;

/// A Chaz/Rika/Rune party on `map` at `(x, y)` with the post-crash flags and
/// `extra` event flags, Snowstorm set so `Event_OutsideRajaTemple` stays quiet.
fn party_at(map: u16, x: u16, y: u16, extra: &[u16]) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(5)),
        Some(CharId(3)),
        None,
        None,
    ]);
    for flag in [0x08, 0x66, 0x68, 0x70, 0x71, 0x72, 0x85, 0x88, SNOWSTORM]
        .iter()
        .chain(extra)
    {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 1,
                    map_index_2: 0,
                    map_index: map,
                    char_x: x * 16,
                    char_y: y * 16,
                },
            })
            .expect("the map loads"),
    )
}

/// Whether the pack carries Tyler's stair chunk (`$47`, `SCENE_CHUNK_WRITES`):
/// a pack built before it was added cannot run the grave scene, and
/// `tests/test_scene_chunk_atlas.py` is what fails on it.
fn tyler_atlas_ready() -> bool {
    let Some(set) = pack() else {
        return false;
    };
    let ready = set
        .data
        .map(psiv_data::MapId(TYLER))
        .and_then(|record| record.patch_tiles.as_ref())
        .is_some_and(|atlas| atlas.tiles.iter().any(|tile| tile.chunk_id == Some(0x47)));
    if !ready {
        eprintln!("the pack has no Tyler chunk $47 atlas; rebuild it. skipping");
    }
    ready
}

fn flag(driver: &Driver, id: u16) -> bool {
    driver.runtime().game().is_set(Flag::event(id))
}

/// Talks to the keeper (object 6, dialogue 48, an `$F6 $5C`) and answers the
/// prompts in order; returns whether `$81` ended up set.
fn gyuna(answers: &[bool]) -> Option<bool> {
    let session = party_at(RYUON_PUB, 38, 30, &[])?;
    let mut driver = Driver::new(session, None);
    assert!(!flag(&driver, GYUNA_TOLD));
    let mut settled = driver.talk(6).expect("the keeper talks");
    for &yes in answers {
        assert_eq!(settled, Settled::Choice, "a prompt is waiting");
        settled = driver.answer(yes).expect("the prompt answers");
    }
    assert_eq!(settled, Settled::Idle, "the conversation ended");
    Some(flag(&driver, GYUNA_TOLD))
}

/// NO, NO, NO to entries 49, 53 and 56, then YES to entry 60 (the space ship):
/// the text stops in entry 60, entry 61 opens on `$35`, and `Event_Gyuna` sets
/// `$81`.
#[test]
fn the_space_ship_answer_sets_the_flag() {
    let Some(set) = gyuna(&[false, false, false, true]) else {
        return;
    };
    assert!(set, "$81: the stop byte was $35");
}

/// The same road with a final NO jumps to entry 62, which ends the tree: the
/// stop byte is not `$35`, and `$81` stays clear.
#[test]
fn declining_the_space_ship_leaves_the_flag_clear() {
    let Some(set) = gyuna(&[false, false, false, false]) else {
        return;
    };
    assert!(!set);
}

/// YES to the storm ends in entry 51: not `$35` either.
#[test]
fn the_storm_answer_leaves_the_flag_clear() {
    let Some(set) = gyuna(&[true]) else {
        return;
    };
    assert!(!set);
}

/// The stair footprint: BG chunk (10,12) is cells (20..21, 24..25).
const STAIRS: [(u16, u16); 4] = [(20, 24), (21, 24), (20, 25), (21, 25)];

fn occupied(driver: &Driver, cell: (u16, u16)) -> bool {
    driver
        .runtime()
        .map()
        .npc_at(Cell::new(cell.0, cell.1))
        .is_some()
}

/// Speak at the grave's middle blocks with `$81` set: the inscription's `$F6`
/// starts `Event_TylerGraveOpening`; both halves of its text play, the twelve
/// objects drift off the stairs on the map, `$84` is set and the stairs' warp
/// (`warp 7`, Hangar) is walkable.
#[test]
fn the_grave_opens_and_the_stairs_lead_to_the_hangar() {
    if !tyler_atlas_ready() {
        return;
    }
    let Some(session) = party_at(TYLER, 20, 27, &[GYUNA_TOLD]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    for cell in STAIRS {
        assert!(occupied(&driver, cell), "{cell:?} is blocked before");
    }
    assert!(!flag(&driver, TYLER_GRAVE));
    driver
        .interact(Cell::new(20, 26), Direction::Up)
        .expect("the grave answers");
    assert!(flag(&driver, TYLER_GRAVE), "$84: the scene ran to its end");
    for cell in STAIRS {
        assert!(!occupied(&driver, cell), "{cell:?} is open after");
    }
    driver
        .go_to_map(0x15F, Some(7))
        .expect("warp 7 now leads to the Hangar");
    assert_eq!(driver.runtime().map_id().0, 0x15F);
}

/// Without `$81` the same press reads the plain inscription and nothing moves:
/// the negative control for the flag gate.
#[test]
fn without_gyunas_word_the_grave_stays_shut() {
    let Some(session) = party_at(TYLER, 20, 27, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver
        .interact(Cell::new(20, 26), Direction::Up)
        .expect("the grave answers");
    assert!(!flag(&driver, TYLER_GRAVE));
    for cell in STAIRS {
        assert!(occupied(&driver, cell), "{cell:?} still blocked");
    }
}
