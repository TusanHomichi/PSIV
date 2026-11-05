//! The Mota Spaceport's five friends driven with pads from hand-built saves
//! (#83). The route reaches the spaceport after the reunion but never takes a
//! fifth, and a chapter here would run most of the game for a side choice. They
//! need the repository pack and skip with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::{Driver, standing_cell};
use psiv_core::{Cell, CharId, Direction, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const MOTA_SPACEPORT: u16 = 0x0BF;
const ELSYDEON: u16 = 0xD9;
const REUNION: u16 = 0xDA;
const HAHN_PICKED: u16 = 0xDB;
const GRYZ_PICKED: u16 = 0xDC;
const KYRA_PICKED: u16 = 0xDF;

const HAHN: CharId = CharId(2);
const GRYZ: CharId = CharId(4);
const KYRA: CharId = CharId(9);

/// The objects: Gryz at `$FFFFC440`, then Hahn, Demi, Raja and Kyra.
const OBJECT_GRYZ: usize = 5;
const OBJECT_HAHN: usize = 6;
const OBJECT_KYRA: usize = 9;

fn at_spaceport(fifth: Option<CharId>, flags: &[u16]) -> Option<Session> {
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
        fifth,
    ]);
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
                    map_index: MOTA_SPACEPORT,
                    char_x: 30 * 16,
                    char_y: 28 * 16,
                },
            })
            .expect("the map loads"),
    )
}

fn active(driver: &Driver, object: usize) -> bool {
    driver.runtime().map().npcs()[object].active
}

fn flag(driver: &Driver, id: u16) -> bool {
    driver.runtime().game().is_set(Flag::event(id))
}

/// `MapDataMan_MotaSpaceport`: who is on the map for each state of the story.
#[test]
fn the_spaceport_shows_who_the_story_has_there() {
    let live = |flags: &[u16], fifth| {
        let session = at_spaceport(fifth, flags)?;
        let driver = Driver::new(session, None);
        Some(
            driver.runtime().map().npcs()[..10]
                .iter()
                .enumerate()
                .filter_map(|(i, npc)| npc.active.then_some(i))
                .collect::<Vec<_>>(),
        )
    };
    let Some(before) = live(&[], None) else {
        return;
    };
    assert_eq!(before, Vec::<usize>::new(), "before Elsydeon, nobody");
    assert_eq!(live(&[ELSYDEON], None).unwrap(), vec![0, 1, 2, 3, 4]);
    assert_eq!(
        live(&[ELSYDEON, REUNION], None).unwrap(),
        vec![5, 6, 7, 8, 9],
        "after the reunion, the five waiting"
    );
    assert_eq!(
        live(&[ELSYDEON, REUNION, HAHN_PICKED], Some(HAHN)).unwrap(),
        vec![5, 7, 8, 9],
        "the one along is not also waiting"
    );
}

/// Hahn joins a party of four, from his own stand: he stands there until the
/// leader walks off, then walks to the party, and is on the trail once there.
#[test]
fn hahn_joins_and_walks_to_the_party() {
    let Some(session) = at_spaceport(None, &[ELSYDEON, REUNION]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let stand = driver.runtime().map().npcs()[OBJECT_HAHN].cell;
    let scenes = driver.scenes_ended();
    driver.talk(OBJECT_HAHN).expect("Hahn asks");
    driver.answer(true).expect("YES");
    assert_eq!(driver.scenes_ended(), scenes + 1, "the event ran out");
    let game = driver.runtime().game();
    assert_eq!(game.party_slot(4), Some(HAHN));
    assert!(flag(&driver, HAHN_PICKED));
    assert!(!active(&driver, OBJECT_HAHN));
    let member = |driver: &Driver, slot: usize| driver.runtime().members()[slot];
    assert_eq!(member(&driver, 4).cell, stand, "at his stand");
    for _ in 0..20 {
        driver.neutral(1).expect("a frame");
    }
    assert_eq!(member(&driver, 4).cell, stand, "until the leader walks");

    // Three cells away and back, then long enough for him to arrive.
    let start = standing_cell(driver.runtime());
    driver
        .go_to(MOTA_SPACEPORT, Cell::new(start.x + 1, start.y))
        .expect("a walk");
    assert_ne!(
        member(&driver, 4).cell,
        stand,
        "the leader's first step set him walking"
    );
    driver.neutral(600).expect("he catches up");
    let ahead = member(&driver, 3).cell;
    let tail = member(&driver, 4).cell;
    let distance = ahead.x.abs_diff(tail.x) + ahead.y.abs_diff(tail.y);
    assert!(distance <= 1, "{tail:?} behind {ahead:?}");
}

/// With Hahn along, Kyra's YES sends him back to his stand and she takes his
/// slot; the flags name only her, and a talk to Hahn starts over.
#[test]
fn kyra_takes_hahns_place() {
    let Some(session) = at_spaceport(Some(HAHN), &[ELSYDEON, REUNION, HAHN_PICKED]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(!active(&driver, OBJECT_HAHN), "along, so not at his stand");
    let hahn_stand = driver.runtime().map().npcs()[OBJECT_HAHN].cell;
    driver.talk(OBJECT_KYRA).expect("Kyra asks");
    driver.answer(true).expect("YES");
    driver.neutral(60).expect("the scene ends");
    let game = driver.runtime().game();
    assert_eq!(game.party_slot(4), Some(KYRA));
    assert!(flag(&driver, KYRA_PICKED));
    assert!(!flag(&driver, HAHN_PICKED));
    assert!(active(&driver, OBJECT_HAHN), "Hahn is back");
    assert_eq!(driver.runtime().map().npcs()[OBJECT_HAHN].cell, hahn_stand);
    assert!(!active(&driver, OBJECT_KYRA));

    // And back: Hahn's dialogue still fires the event.
    driver.talk(OBJECT_HAHN).expect("Hahn asks");
    driver.answer(true).expect("YES");
    driver.neutral(60).expect("the scene ends");
    assert_eq!(driver.runtime().game().party_slot(4), Some(HAHN));
    assert!(active(&driver, OBJECT_KYRA), "Kyra is back");
}

/// NO changes nobody.
#[test]
fn no_leaves_the_party_as_it_was() {
    let Some(session) = at_spaceport(Some(HAHN), &[ELSYDEON, REUNION, HAHN_PICKED]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(OBJECT_GRYZ).expect("Gryz asks");
    driver.answer(false).expect("NO");
    driver.neutral(60).expect("the scene ends");
    assert_eq!(driver.runtime().game().party_slot(4), Some(HAHN));
    assert!(!flag(&driver, GRYZ_PICKED));
    assert!(active(&driver, OBJECT_GRYZ));
    assert!(driver.runtime().game().party_slot(4) != Some(GRYZ));
    let leader = driver.runtime().members()[0].facing;
    let gryz = driver.runtime().map().npcs()[OBJECT_GRYZ].facing;
    assert_eq!(gryz, leader.opposite(), "he turned to the leader");
    let _: Direction = gryz;
}
