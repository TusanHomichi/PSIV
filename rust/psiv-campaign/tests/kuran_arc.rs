//! The Hangar, the Dezo spaceport and Kuran's third floor, driven with pads from
//! hand-built saves (lane C6).
//!
//! The whole arc, Landale to the defeat of Dark Force 1, is the route's
//! `dezolis-landale` .. `kuran-dark-force-1` chapters; the ignored whole-route
//! test in `runner.rs` plays them from New Game. These tests keep the seams that
//! route leans on cheap enough for the ordinary suite: a trigger on a warp
//! footprint (`step_onto`), the spaceport's destination list, Kuran's arrival,
//! and the two F3 triggers around the event battle. They need the repository
//! pack and skip with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_campaign::halt::HaltKind;
use psiv_core::{
    Cell, CharId, CollisionType, Direction, Flag, GameState, RetailLocation, RetailSave,
};
use psiv_runtime::Session;

const HANGAR: u16 = 0x15F;
const DEZOLIS: u16 = 0x001;
const SPACEPORT: u16 = 0x0D4;
const KURAN: u16 = 0x190;
const KURAN_F3: u16 = 0x198;
const ZELAN_F1: u16 = 0x18E;

const DEZO_SPACEPORT: u16 = 0x82;
const KURAN_ARRIVED: u16 = 0x86;
const NEAR_DARK_FORCE: u16 = 0x87;
const DARK_FORCE_1: u16 = 0x83;
const ICE_DIGGER_FLAG: u16 = 0x89;

/// Chest-free item ids (`ps4.constants.asm`): the Canceller the cutscene takes
/// and the Ice Digger it gives.
const ITEM_CANCELLER: u8 = 0x9A;
const ITEM_ICE_DIGGER: u8 = 0x97;

/// The Landale door: the four cells of BG chunk (6,36) on Dezolis.
const SPACEPORT_DOOR: [(u16, u16); 4] = [(12, 72), (13, 72), (12, 73), (13, 73)];

/// A Chaz/Rika/Rune party on `map` at `(x, y)` in world `world`, with the
/// post-grave flags (`$84` among them) and `extra` event flags.
fn party_at(map: u16, world: u16, x: u16, y: u16, extra: &[u16]) -> Option<Session> {
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
    for flag in [
        0x08, 0x66, 0x68, 0x70, 0x71, 0x72, 0x80, 0x81, 0x84, 0x85, 0x88,
    ]
    .iter()
    .chain(extra)
    {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    game.inventory_mut()
        .add(ITEM_CANCELLER)
        .expect("room for the Canceller");
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: world,
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

/// The Landale row is the first row of the Hangar's warp 0, and `RunEvents`
/// runs before `RunMapTransitions` on foot: `step_onto` takes the one step,
/// the scene starts, raises the spaceport on the live map and leaves the party
/// on Dezolis with `$82` set.
#[test]
fn step_onto_the_landale_row_starts_the_scene_and_raises_the_spaceport() {
    let Some(session) = party_at(HANGAR, 1, 26, 82, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(!flag(&driver, DEZO_SPACEPORT));
    driver
        .step_onto(HANGAR, Cell::new(26, 83))
        .expect("the trigger on the warp footprint starts Cutscene_Landale");
    assert!(flag(&driver, DEZO_SPACEPORT), "the scene ended and set $82");
    assert_eq!(driver.map(), DEZOLIS, "the scene loaded Dezolis");
    assert_eq!((driver.cell().x, driver.cell().y), (9, 74));
    assert!(driver.scenes_ended() > 0);
    for (x, y) in SPACEPORT_DOOR {
        assert_eq!(
            driver.runtime().map().collision_at(Cell::new(x, y)),
            Some(CollisionType::MapChange),
            "the spaceport door ({x},{y}) is on the live map"
        );
    }
}

/// The negative controls of `step_onto`: with `$82` set the trigger's gate is
/// shut and the very step fires the warp, which the objective reports as a
/// halt; a cell no step reaches by firing a warp is unreachable by name.
#[test]
fn step_onto_halts_when_the_warp_fires_or_no_scene_runs_on_the_cell() {
    let Some(session) = party_at(HANGAR, 1, 26, 82, &[DEZO_SPACEPORT]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let halt = driver
        .step_onto(HANGAR, Cell::new(26, 83))
        .expect_err("with $82 set the step takes the warp");
    assert_eq!(halt.kind, HaltKind::UnexpectedState);
    assert!(halt.detail.contains("fired the warp"), "{halt:?}");
    assert_eq!(driver.map(), DEZOLIS, "the warp took the party to Dezolis");

    let Some(session) = party_at(HANGAR, 1, 26, 82, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let halt = driver
        .step_onto(HANGAR, Cell::new(26, 81))
        .expect_err("(26,81) is open ground where no trigger fires");
    assert_eq!(halt.kind, HaltKind::UnexpectedState);
    assert!(halt.detail.contains("no scene ran"), "{halt:?}");
    assert!(!flag(&driver, DEZO_SPACEPORT));
    let halt = driver
        .step_onto(HANGAR, Cell::new(0, 0))
        .expect_err("(0,0) is a wall no walk ends on");
    assert_eq!(halt.kind, HaltKind::Unreachable);
}

/// With the spaceport raised (`$82`) the destination list at the Dezo
/// spaceport names Kuran, and the arrival's map trigger runs
/// `Event_KuranArrival` on the first field frame. Without `$82` the list has
/// no Kuran row, so the same request is a halt by name.
#[test]
fn the_spaceport_flies_to_kuran_only_once_it_has_appeared() {
    let Some(session) = party_at(SPACEPORT, 1, 30, 20, &[DEZO_SPACEPORT]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(!flag(&driver, KURAN_ARRIVED));
    driver
        .board(Direction::Up, 4)
        .expect("Kuran is on the list and flies");
    assert_eq!(driver.map(), KURAN);
    assert_eq!(driver.runtime().world_index(), 4, "the flight wrote Kuran");
    assert!(flag(&driver, KURAN_ARRIVED), "Event_KuranArrival set $86");

    let Some(session) = party_at(SPACEPORT, 1, 30, 20, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let halt = driver
        .board(Direction::Up, 4)
        .expect_err("the unraised spaceport's list has no Kuran");
    assert_eq!(halt.kind, HaltKind::MenuEntryMissing);
    assert!(halt.detail.contains("lists ["), "{halt:?}");
    assert!(!flag(&driver, KURAN_ARRIVED));
}

/// F3's corridor: row 33 (`y $200`) fires `Event_NearDarkForce1`, row 14
/// (`y $0D0`) fires `Event_DarkForce1`, which sets `$83` and starts event
/// battle 9. No policy is set here, so the battle's start is the halt.
#[test]
fn the_two_f3_rows_fire_their_events_and_the_second_starts_battle_9() {
    let Some(session) = party_at(KURAN_F3, 4, 30, 40, &[DEZO_SPACEPORT, KURAN_ARRIVED]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(!flag(&driver, NEAR_DARK_FORCE));
    driver
        .go_to(KURAN_F3, Cell::new(30, 33))
        .expect("the corridor walks");
    assert!(flag(&driver, NEAR_DARK_FORCE), "row 33 set $87");
    assert!(!flag(&driver, DARK_FORCE_1), "Dark Force 1 is still ahead");
    let halt = driver
        .go_to(KURAN_F3, Cell::new(30, 14))
        .expect_err("the event battle starts with no policy to fight it");
    assert_eq!(halt.kind, HaltKind::UnexpectedState);
    assert!(halt.detail.contains("no policy"), "{halt:?}");
    assert!(
        flag(&driver, DARK_FORCE_1),
        "row 14 set $83 before the battle"
    );
    let battle = driver.battles().last().expect("a battle began");
    assert_eq!((battle.kind, battle.id), ("event", 9));
}

/// With `$83` set and `$89` clear, the first field frame fires
/// `RunEvent_DarkForce1Defeated`: Zelan F1 at (31,17), `$89` set, the Canceller
/// swapped for the Ice Digger. Without `$83` nothing fires.
#[test]
fn dark_force_1_defeated_takes_the_party_to_zelan_and_swaps_the_items() {
    let extra = [DEZO_SPACEPORT, KURAN_ARRIVED, NEAR_DARK_FORCE, DARK_FORCE_1];
    let Some(session) = party_at(KURAN_F3, 4, 30, 14, &extra) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.settle(false).expect("the cutscene plays out");
    assert!(flag(&driver, ICE_DIGGER_FLAG), "$89 is set");
    assert_eq!(driver.map(), ZELAN_F1);
    assert_eq!((driver.cell().x, driver.cell().y), (31, 17));
    let game = driver.runtime().game();
    assert!(game.inventory().contains(ITEM_ICE_DIGGER), "the Ice Digger");
    assert!(!game.inventory().contains(ITEM_CANCELLER), "no Canceller");

    // Negative control: the battle not fought (and the party below the row that
    // would start it), the scene stays quiet.
    let quiet = [DEZO_SPACEPORT, KURAN_ARRIVED, NEAR_DARK_FORCE];
    let Some(session) = party_at(KURAN_F3, 4, 30, 20, &quiet) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.settle(false).expect("nothing to play");
    assert!(!flag(&driver, ICE_DIGGER_FLAG));
    assert_eq!(driver.map(), KURAN_F3);
}
