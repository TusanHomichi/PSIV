//! The Plate System's events driven with pads from hand-built saves (#81):
//! Phonon from Plate System F3's chest and the engine on F4. They need the
//! repository pack and skip with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{Cell, CharId, Direction, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const PLATE_SYSTEM_F3: u16 = 0x0BD;
const PLATE_SYSTEM_F4: u16 = 0x0BE;
const CHAZ: CharId = CharId(0);
const GRYZ: CharId = CharId(4);
const RIKA: CharId = CharId(5);
const DEMI: CharId = CharId(6);

fn party_at(map: u16, x: u16, y: u16, members: [Option<CharId>; 5]) -> Option<Driver> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party(members);
    let session = Session::start(set.data.clone())
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
        .expect("the map loads");
    Some(Driver::new(session, None))
}

/// The chest at `(27,20)` (object 3 after the floor's three elevators):
/// interaction area 3 fires `$6C`. With Demi she takes the unit; without her
/// the chest closes and its flag is clear again.
#[test]
fn the_phonon_chest_waits_for_demi() {
    let with = [Some(CHAZ), Some(RIKA), Some(DEMI), None, None];
    let Some(mut driver) = party_at(PLATE_SYSTEM_F3, 27, 22, with) else {
        return;
    };
    let scenes = driver.scenes_ended();
    driver
        .interact(Cell::new(27, 21), Direction::Up)
        .expect("the chest opens");
    assert_eq!(driver.scenes_ended(), scenes + 1, "Event_Phonon ran");
    let game = driver.runtime().game();
    assert_eq!(game.roster().get(DEMI).unwrap().skills[5], 0x11);

    let without = [Some(CHAZ), Some(RIKA), Some(GRYZ), None, None];
    let Some(mut driver) = party_at(PLATE_SYSTEM_F3, 27, 22, without) else {
        return;
    };
    driver
        .interact(Cell::new(27, 21), Direction::Up)
        .expect("the chest opens");
    assert!(!driver.runtime().game().is_set(Flag::chest(0x42)));
    assert_eq!(driver.runtime().map().npcs()[3].facing, Direction::Down);
}

/// The engine's console row on F4 (areas 1 to 3 at row 23, gated on
/// `EventFlag_PlateEngine` `$61` clear) fires `$2D` once.
#[test]
fn the_engine_is_read_once() {
    let members = [Some(CHAZ), Some(GRYZ), Some(DEMI), Some(RIKA), None];
    let Some(mut driver) = party_at(PLATE_SYSTEM_F4, 30, 26, members) else {
        return;
    };
    let scenes = driver.scenes_ended();
    driver
        .interact(Cell::new(30, 24), Direction::Up)
        .expect("the engine");
    assert_eq!(
        driver.scenes_ended(),
        scenes + 1,
        "Event_PlateSysEngine ran"
    );
    assert!(driver.runtime().game().is_set(Flag::event(0x61)));

    // Negative control: the flag closes the areas.
    let scenes = driver.scenes_ended();
    driver
        .interact(Cell::new(30, 24), Direction::Up)
        .expect("nothing to read");
    assert_eq!(driver.scenes_ended(), scenes);
}
