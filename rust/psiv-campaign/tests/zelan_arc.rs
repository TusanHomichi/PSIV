//! Zelan F1 and the boarding row, driven with pads from hand-built saves (lane C5).
//!
//! The whole arc, Wren to the crash landing, is the route's `zelan-wren-canceller`,
//! `zelan-sabotage` and `dezolis-first-control` chapters; the ignored
//! whole-route test in `runner.rs` plays them from New Game. These tests keep
//! the two seams that route leans on cheap enough for the ordinary suite: the
//! Wren talk and the Canceller chest (`MapUpdate_ZelanCanceller` writing `$72`),
//! and the sabotage row's one-row Kuran menu. A party built here is too weak
//! for the Chaos Sorcerer, so the fight itself is the route's.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_campaign::halt::HaltKind;
use psiv_core::{CharId, Direction, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const ZELAN: u16 = 0x18D;
const ZELAN_F1: u16 = 0x18E;
const WREN_JOINED: u16 = 0x70;
const CHAOS_SORCERER: u16 = 0x71;
const CANCELLER_HELD: u16 = 0x72;

/// A Chaz/Rika/Rune party on `map` at cell `(x, y)` with the post-Zio flags and
/// `extra` event flags.
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
    for flag in [0x08, 0x66, 0x68].iter().chain(extra) {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0x0300,
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

/// Wren joins on F1 and the Canceller chest, opened afterwards, makes a field
/// frame's map update set `$72`: the flag the sabotage trigger waits on (H25).
#[test]
fn wren_joins_and_the_canceller_chest_sets_the_flag_the_sabotage_waits_on() {
    let Some(session) = party_at(ZELAN_F1, 31, 14, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(!flag(&driver, WREN_JOINED));
    driver.talk(0).expect("Wren talks");
    assert!(flag(&driver, WREN_JOINED), "Cutscene_MeetingWren sets $70");
    assert_eq!(driver.runtime().game().party_members().len(), 4);
    assert!(
        !flag(&driver, CANCELLER_HELD),
        "the chest is shut: nothing has set $72"
    );
    driver
        .open_chest(5)
        .expect("the Canceller chest is reachable and opens");
    driver.neutral(30).expect("field frames pass");
    assert!(
        flag(&driver, CANCELLER_HELD),
        "MapUpdate_ZelanCanceller sets $72 once chest flag $0B is set"
    );
}

/// With Wren and the Canceller held, the step onto Zelan's boarding row fires
/// the sabotage scene, whose destination menu lists Kuran alone (mask `$08`);
/// a world it does not list is a halt by name.
#[test]
fn the_sabotage_row_opens_a_menu_that_lists_kuran_alone() {
    let Some(session) = party_at(ZELAN, 30, 47, &[WREN_JOINED, CANCELLER_HELD]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(!flag(&driver, CHAOS_SORCERER));
    let halt = driver
        .board(Direction::Down, 3)
        .expect_err("Zelan is not on the sabotage's list");
    assert_eq!(halt.kind, HaltKind::MenuEntryMissing);
    assert!(halt.detail.contains("lists [4]"), "{halt:?}");
    assert!(
        !flag(&driver, CHAOS_SORCERER),
        "nothing flew: the Chaos Sorcerer flag is still clear"
    );
    assert_ne!(driver.runtime().world_index(), 4, "no confirm wrote Kuran");
}

/// Without the Canceller the same step starts the ordinary boarding
/// (`RunEvent_KuranEnterSpaceship`, `$23`) and not the sabotage: its menu lists
/// Motavia alone, so asking for Kuran is a halt (the negative control for the
/// chest order the route's Wren chapter relies on).
#[test]
fn the_sabotage_needs_the_canceller_flag() {
    let Some(session) = party_at(ZELAN, 30, 47, &[WREN_JOINED]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let halt = driver
        .board(Direction::Down, 4)
        .expect_err("the ordinary boarding does not list Kuran here");
    assert_eq!(halt.kind, HaltKind::MenuEntryMissing);
    assert!(halt.detail.contains("lists [0]"), "{halt:?}");
    assert!(!flag(&driver, CHAOS_SORCERER));
}
