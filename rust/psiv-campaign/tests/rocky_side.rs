//! Rocky (#81) on the pack, on hand-built saves: the route does not take the
//! guild's jobs. Skips without the local pack.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const KRUP: u16 = 0x039;
const ROCKY_HOUSE: u16 = 0x065;
const MONSEN: u16 = 0x073;
const TERMI: u16 = 0x07B;
const SHORTCAKE: u8 = 0x91;

/// The dog job taken and his owner spoken to.
const JOB: [u16; 2] = [0x1E, 0x1F];

/// A Chaz/Alys party on `map` at cell `(x, y)` with `flags`, holding `cakes`.
fn party_at(map: u16, x: u16, y: u16, flags: &[u16], cakes: usize) -> Option<Session> {
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
    for _ in 0..cakes {
        game.inventory_mut().add(SHORTCAKE).expect("room");
    }
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: map,
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

/// Tree 12 entry `$28`: neither `$22` nor `$20` set, so `$F6 $73`. The flag
/// hides him on Monsen (`MapDataMan_MonsenRocky`) at the next load.
#[test]
fn he_is_found_at_monsen_after_the_owner_asks() {
    let Some(session) = party_at(MONSEN, 38, 46, &JOB, 0) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(driver.runtime().map().npcs()[7].active, "$1F shows him");
    driver.talk(7).expect("he speaks");
    assert!(flag(&driver, 0x20), "Event_RockyFound");
    assert!(!flag(&driver, 0x23));

    // Negative control: before the owner he is not on the map.
    let Some(session) = party_at(MONSEN, 38, 46, &[0x1E], 0) else {
        return;
    };
    let driver = Driver::new(session, None);
    assert!(!driver.runtime().map().npcs()[7].active);
}

/// The chase's order on the map effects: found at Monsen, he is at Termi; from
/// Termi he runs to Krup; from Krup to Monsen; each without a shortcake.
#[test]
fn he_runs_termi_to_krup_to_monsen() {
    let found = [JOB[0], JOB[1], 0x20];
    let Some(session) = party_at(TERMI, 38, 35, &found, 0) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(driver.runtime().map().npcs()[9].active, "at Termi");
    driver.talk(9).expect("he speaks and runs");
    assert!(flag(&driver, 0x21), "the Termi escape");
    assert!(!driver.runtime().map().npcs()[9].active, "and he is gone");

    let termi = [JOB[0], JOB[1], 0x20, 0x21];
    let Some(session) = party_at(KRUP, 44, 34, &termi, 0) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(driver.runtime().map().npcs()[11].active, "at Krup");
    driver.talk(11).expect("he speaks and runs");
    assert!(flag(&driver, 0x22), "the Krup escape");
    assert!(!flag(&driver, 0x21), "only the last map's flag stays");

    let krup = [JOB[0], JOB[1], 0x20, 0x22];
    let Some(session) = party_at(MONSEN, 38, 46, &krup, 0) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(driver.runtime().map().npcs()[7].active, "at Monsen again");
    driver.talk(7).expect("he speaks and runs");
    assert!(flag(&driver, 0x23), "the Monsen escape");
    assert!(!flag(&driver, 0x22));
}

/// With two shortcakes he is caught at Termi: both go, `$24` (the guild job's
/// `earned`) goes on, and the party stands in his house, with him there.
#[test]
fn a_shortcake_brings_him_home() {
    let found = [JOB[0], JOB[1], 0x20];
    let Some(session) = party_at(TERMI, 38, 35, &found, 2) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(9).expect("he is caught");
    assert!(flag(&driver, 0x24), "Event_CatchingRocky");
    assert_eq!(driver.runtime().map_id().0, ROCKY_HOUSE);
    assert!(!driver.runtime().game().inventory().contains(SHORTCAKE));
    assert!(driver.runtime().map().npcs()[3].active, "the dog at home");
}
