//! The Wreckage's system driven with pads from a hand-built save (#81). It
//! needs the repository pack and skips with a message without one; a pack
//! built before `SCENE_CHUNK_WRITES` carried the Wreckage's row cannot write
//! the floor and fails here (rebuild it, `docs/DEVELOPMENT.md`).

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{Cell, CharId, Direction, Flag, GameState, PixelPos, RetailLocation, RetailSave};
use psiv_runtime::Session;

const WRECKAGE_F2_PART4: u16 = 0x0B6;
const CHAZ: CharId = CharId(0);
const RIKA: CharId = CharId(5);
const DEMI: CharId = CharId(6);

fn party_at(x: u16, y: u16, flags: &[u16]) -> Option<Driver> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CHAZ), Some(RIKA), Some(DEMI), None, None]);
    for flag in flags {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    let session = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: WRECKAGE_F2_PART4,
                char_x: x * 16,
                char_y: y * 16,
            },
        })
        .expect("the map loads");
    Some(Driver::new(session, None))
}

/// The console row (areas 0 to 2 at row 33) fires `$29`: Rika reads it, the
/// floor lights and `EventFlag_WreckageSystem` is set; a second read speaks
/// entry `$48` and changes nothing.
#[test]
fn rika_reads_the_wreckages_system() {
    let Some(mut driver) = party_at(30, 36, &[]) else {
        return;
    };
    // Chunk columns 7 and 22 on rows 12 and 13: the first pass's and the
    // last pass's outer writes.
    let floor = |driver: &Driver| {
        [(15, 12), (14, 13), (7, 12), (22, 13)].map(|(cx, cy): (i32, i32)| {
            driver.runtime().map_chunk_at(PixelPos {
                x: cx * 32,
                y: cy * 32,
            })
        })
    };
    assert!(
        floor(&driver).iter().all(|chunk| *chunk != Some(0xA6)),
        "dark before: {:?}",
        floor(&driver)
    );
    let scenes = driver.scenes_ended();
    driver
        .interact(Cell::new(30, 34), Direction::Up)
        .expect("the system");
    assert_eq!(floor(&driver), [Some(0xA6); 4], "the floor lit");
    assert_eq!(
        driver.scenes_ended(),
        scenes + 1,
        "Event_WreckageEngine ran"
    );
    assert!(driver.runtime().game().is_set(Flag::event(0x40)));
    assert_eq!(
        psiv_campaign::driver::standing_cell(driver.runtime()),
        Cell::new(30, 34),
        "the leader is back where the read began"
    );

    let scenes = driver.scenes_ended();
    driver
        .interact(Cell::new(30, 34), Direction::Up)
        .expect("the line again");
    assert_eq!(driver.scenes_ended(), scenes + 1);
}
