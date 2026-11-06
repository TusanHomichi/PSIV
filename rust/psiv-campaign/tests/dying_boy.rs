//! Torinco's dying boy driven with pads from a hand-built save (#81): the
//! Alis Sword cures him (`$86`). It needs the repository pack and skips with
//! a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const CULVERS_HOUSE: u16 = 0x0E7;
/// `EventFlag_DyingBoy` (the job), `EventFlag_Culvers` (the father asked).
const JOB_AND_FATHER: [u16; 2] = [0xA8, 0xA9];
const ALIS_SWORD: u16 = 0xAA;
const ITEM_ALIS_SWORD: u8 = 0x8A;
/// `CulversHouse` object 4, the invisible block at the bed's right edge that
/// carries the boy's dialogue (`$32`, which reaches `$F6 $00 $86`). The boy's
/// own object 2 is not talkable, and block 3 is walled in by the bed, the
/// mother and the boy.
const BED_BLOCK: usize = 4;

/// Chaz, Rune, Rika, Wren and Raja in Culvers's house at cell `(x, y)`.
fn house(x: u16, y: u16, flags: &[u16], items: &[u8]) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(3)),
        Some(CharId(5)),
        Some(CharId(7)),
        Some(CharId(8)),
    ]);
    for flag in flags {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    for item in items {
        game.inventory_mut().add(*item).expect("room in the bag");
    }
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: CULVERS_HOUSE,
                    char_x: x * 16,
                    char_y: y * 16,
                },
            })
            .expect("the map loads"),
    )
}

fn holds_sword(driver: &Driver) -> bool {
    driver
        .runtime()
        .game()
        .inventory()
        .slots()
        .contains(&ITEM_ALIS_SWORD)
}

/// With the job taken and the father asked, talking at the bed with the sword
/// runs the cure: the sword is taken and `EventFlag_AlisSword` is set.
#[test]
fn the_alis_sword_cures_the_boy() {
    let Some(session) = house(26, 31, &JOB_AND_FATHER, &[ITEM_ALIS_SWORD]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.set_budget(8_000);
    let scenes = driver.scenes_ended();
    driver.talk(BED_BLOCK).expect("the cure plays");
    assert_eq!(driver.scenes_ended(), scenes + 1, "Event_DyingBoy ran");
    assert!(
        driver.runtime().game().is_set(Flag::event(ALIS_SWORD)),
        "Event_DyingBoy"
    );
    assert!(!holds_sword(&driver), "the sword is taken");
}

/// Negative controls: without the father's word, or without the sword, the
/// boy only talks and nothing is written.
#[test]
fn no_father_or_no_sword_writes_nothing() {
    for (flags, items) in [
        (&[0xA8][..], &[ITEM_ALIS_SWORD][..]),
        (&JOB_AND_FATHER[..], &[][..]),
    ] {
        let Some(session) = house(26, 31, flags, items) else {
            return;
        };
        let mut driver = Driver::new(session, None);
        let before = driver.runtime().game().snapshot();
        driver.talk(BED_BLOCK).expect("he speaks");
        assert!(!driver.runtime().game().is_set(Flag::event(ALIS_SWORD)));
        assert_eq!(driver.runtime().game().snapshot(), before);
    }
}
