//! The Mota Spaceport's boarding row, walked with pads (lane C4).
//!
//! The boarding row is a type-1 collision cell whose job is an event, not a
//! warp: the cartridge's `RunEvents` runs before `RunMapTransitions`
//! (`ps4.asm:116768-116773`), so stepping on it starts `Cutscene_InsideSpaceship`
//! and the map-change table is never read. The runtime still reports that cell
//! as unmapped in the frame the scene starts; the runner must not halt on it.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const MOTA_SPACEPORT: u16 = 0xBF;

/// A Chaz/Rika/Rune party standing on row `y` of the spaceport's boarding corridor
/// (column 30), with the flags the post-Zio story holds on arrival.
fn at_the_gangway(y: u16) -> Option<Session> {
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
    for flag in [0x08, 0x66, 0x68] {
        game.set(Flag::event(flag)).expect("flag in range");
    }
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: MOTA_SPACEPORT,
                    char_x: 30 * 16,
                    char_y: y * 16,
                },
            })
            .expect("the spaceport loads"),
    )
}

/// One step north onto the boarding row starts the ship's scene; the walk ends
/// in the story's hands instead of halting on a scene fault.
#[test]
fn stepping_onto_the_boarding_row_is_a_scene_and_not_a_fault() {
    let Some(session) = at_the_gangway(20) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert_eq!(driver.map(), MOTA_SPACEPORT);
    let start = driver.cell();
    assert_eq!((start.x, start.y), (30, 20), "the foot of the boarding row");
    driver
        .go_to(MOTA_SPACEPORT, psiv_core::Cell { x: 30, y: 19 })
        .expect("the boarding step is not a halt");
    assert!(
        driver.scenes_ended() > 0 || driver.cell().y == 19,
        "the step either ran the scene or stands on the row"
    );
}

/// Row 18 above the boarding row is wall: the planner refuses it by name, so
/// the first test's walk is to the last open cell and not past it.
#[test]
fn the_wall_above_the_boarding_row_is_unreachable() {
    let Some(session) = at_the_gangway(20) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let halt = driver
        .go_to(MOTA_SPACEPORT, psiv_core::Cell { x: 30, y: 18 })
        .expect_err("row 18 is wall");
    assert_eq!(halt.kind, psiv_campaign::halt::HaltKind::Unreachable);
}
