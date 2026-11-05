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

/// The engine's light record names `loc_6E908`, whose words come from the
/// pack (`presentation/panels.json`, `palettes`), not from source. Expanded as
/// the cartridge's loop reads them, they give sixteen writes, and the sixth
/// and twelfth take their second word from past the table: the window's last
/// word, the first of `loc_6E922`'s code.
#[test]
fn the_engine_light_expands_from_the_pack() {
    use psiv_core::{EventIndex, PresentationOp, SceneOp, palette_cycle_writes, scene_for};

    let dir = common::pack_dir();
    let Ok(text) = std::fs::read_to_string(dir.join("presentation/panels.json")) else {
        eprintln!("runtime pack not present at {}; skipping", dir.display());
        return;
    };
    let scene = scene_for(EventIndex(0x2D)).expect("registered");
    let (source, second_skip, steps) = scene
        .ops
        .iter()
        .find_map(|op| match op {
            SceneOp::Presentation {
                op:
                    PresentationOp::PaletteCycleFromTable {
                        source_rom_addr,
                        second_skip,
                        steps,
                        ..
                    },
            } => Some((*source_rom_addr, *second_skip, *steps)),
            _ => None,
        })
        .expect("the light record");
    let manifest: serde_json::Value = serde_json::from_str(&text).expect("panels.json parses");
    let record = manifest["palettes"]
        .as_array()
        .expect("palettes")
        .iter()
        .find(|r| r["rom_addr"] == format!("0x{source:06X}"))
        .expect("the pack carries the light's table");
    let hex = record["raw_hex"].as_str().expect("raw_hex");
    let words: Vec<u16> = (0..hex.len() / 4)
        .map(|i| u16::from_str_radix(&hex[4 * i..4 * i + 4], 16).expect("hex"))
        .collect();
    assert_eq!(words.len(), record["words"].as_u64().unwrap() as usize);

    let writes = palette_cycle_writes(&words, second_skip, steps).expect("every read fits");
    assert_eq!(writes.len(), 16);
    let terminator = words
        .iter()
        .position(|w| w & 0x8000 != 0)
        .expect("a terminator");
    let colours = &words[..terminator];
    let last = *words.last().unwrap();
    for (pass, (first, second)) in writes.iter().enumerate() {
        assert_eq!(*first, colours[pass % colours.len()], "pass {pass}");
        let past = pass % colours.len() == colours.len() - 1;
        assert_eq!(*second == last, past, "pass {pass} reads past the table");
    }
    // Negative control: a window one word short cannot serve those reads.
    assert_eq!(
        palette_cycle_writes(&words[..words.len() - 1], second_skip, steps),
        None
    );
}
