//! The ranch owner at Mile (#81) on the pack, on hand-built saves. Skips
//! without the local pack.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{Cell, CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const MILE: u16 = 0x01D;
const OWNER: usize = 0;

fn party_at(x: u16, y: u16, flags: &[u16]) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    for flag in flags {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: MILE,
                    char_x: x * 16,
                    char_y: y * 16,
                },
            })
            .expect("the map loads"),
    )
}

fn flag(driver: &Driver, id: u16) -> bool {
    driver.runtime().game().is_set(Flag::event(id))
}

/// Tree 3 entry `$11`: `$19` (the guild's job) set reaches `$F6 $70`. He ends
/// at (`$110,$2D0`), the two blocks gone, and `$1A` is set. A fresh load with
/// `$1A` set places him there too, through `MapDataMan_RanchOwner`.
#[test]
fn the_owner_stands_aside_and_stays_there_on_a_reload() {
    let Some(session) = party_at(19, 48, &[0x19]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let map = driver.runtime().map();
    assert_eq!(map.npcs()[OWNER].cell, Cell::new(19, 45));
    assert!(map.npcs()[8].active && map.npcs()[9].active);
    driver.talk(OWNER).expect("he speaks");
    assert!(flag(&driver, 0x1A), "Event_RanchOwner");
    let map = driver.runtime().map();
    assert_eq!(map.npcs()[OWNER].cell, Cell::new(0x11, 0x2E));
    assert!(!map.npcs()[8].active && !map.npcs()[9].active);

    let Some(session) = party_at(19, 48, &[0x19, 0x1A]) else {
        return;
    };
    let driver = Driver::new(session, None);
    let map = driver.runtime().map();
    assert_eq!(map.npcs()[OWNER].cell, Cell::new(0x11, 0x2E));
    assert_eq!(map.npcs()[OWNER].id.0, 0x38);
    assert!(!map.npcs()[8].active && !map.npcs()[9].active);

    // Negative control: no job, no scene, and he stays.
    let Some(session) = party_at(19, 48, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(OWNER).expect("he speaks");
    assert!(!flag(&driver, 0x1A));
    assert!(driver.runtime().map().npcs()[8].active);
}

/// With the worm dead (`$1B`) and `$1C` clear: `$F6 $72`.
#[test]
fn after_the_worm_he_thanks_the_party() {
    let Some(session) = party_at(17, 48, &[0x19, 0x1A, 0x1B]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(OWNER).expect("he speaks");
    assert!(flag(&driver, 0x1C), "Event_RanchOwnerAfterBattle");
}
