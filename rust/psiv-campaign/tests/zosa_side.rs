//! Zosa's item shop driven with pads from a hand-built save (#56): the
//! thief takes the Pengu Feed (`$96`). It needs the repository pack and skips
//! with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const ZOSA_ITEM_SHOP: u16 = 0x12A;
const PENGU_FEED: u8 = 0x92;
/// `$FFFFC380`, past the shop's two objects.
const THIEF: usize = 2;
/// The standing row `RunEvent_PenguFeedStolen` asks for, `y == $260`: the
/// cell below the one the pixel names (`PixelPos::from_cell`).
const TRIGGER_ROW: u16 = 0x260 / 16 + 1;

/// Chaz alone in the shop standing on cell `(x, y)`, carrying `items`.
fn shop(x: u16, y: u16, items: &[u8]) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), None, None, None, None]);
    for item in items {
        game.inventory_mut().add(*item).expect("room in the bag");
    }
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 1,
                    map_index_2: 0,
                    map_index: ZOSA_ITEM_SHOP,
                    char_x: x * 16,
                    char_y: y * 16,
                },
            })
            .expect("the map loads"),
    )
}

fn feeds(driver: &Driver) -> usize {
    driver
        .runtime()
        .game()
        .inventory()
        .slots()
        .iter()
        .filter(|&&id| id == PENGU_FEED)
        .count()
}

/// Standing on the row with two Feeds in the bag: the scene runs, the thief
/// is placed in slot 2 and cleared again, and both Feeds are gone.
#[test]
fn the_thief_takes_every_pengu_feed() {
    let Some(session) = shop(30, TRIGGER_ROW, &[PENGU_FEED, 0x01, PENGU_FEED]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.set_budget(4_000);
    let scenes = driver.scenes_ended();
    for _ in 0..40 {
        driver.neutral(30).expect("the scene plays");
        if driver.scenes_ended() > scenes && !driver.runtime().scene_active() {
            break;
        }
        // Close each line as it comes up.
        let _ = driver.tap(psiv_runtime::Button::Speak);
    }
    assert_eq!(
        driver.scenes_ended(),
        scenes + 1,
        "Event_PenguinFeedStolen ran"
    );
    assert_eq!(feeds(&driver), 0, "both Feeds taken");
    assert!(
        driver.runtime().game().inventory().slots().contains(&0x01),
        "the rest of the bag stays"
    );
    let objects = driver.runtime().map().npcs();
    assert!(objects.len() > THIEF, "slot 2 was written");
    assert!(!objects[THIEF].active, "and cleared again");
    assert_eq!(
        objects[THIEF].cell.x,
        0x2D0 / 16,
        "where the second run ends"
    );
}

/// Negative control: with no Feed in the bag the row fires nothing.
#[test]
fn no_feed_no_thief() {
    let Some(session) = shop(30, TRIGGER_ROW, &[0x01]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    driver.neutral(120).expect("the field runs");
    assert_eq!(driver.scenes_ended(), scenes);
    assert_eq!(driver.runtime().map().npcs().len(), 2, "no slot written");
}
