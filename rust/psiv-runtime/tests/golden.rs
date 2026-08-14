//! Golden-path test: the slice's acceptance criterion, headless.
//!
//! Spawn in Piata below the academy doorway, walk up, and assert the warp
//! fires into PiataAcademy at the exact cell/facing the cartridge's
//! transition table stores. Gated on the runtime pack, which is generated
//! locally and never committed.

use std::path::Path;

use psiv_core::{Cell, Direction, Input, MapId, StepFrames, WarpTrigger};
use psiv_data::GameData;
use psiv_runtime::{Runtime, RuntimeEvent};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn load() -> Option<GameData> {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present at {PACK}; skipping golden test");
        return None;
    }
    Some(GameData::load(Path::new(PACK)).expect("pack loads and validates"))
}

#[test]
fn walking_into_the_academy_warps_to_the_academy() {
    let Some(data) = load() else { return };

    // The doorway rect is (31..=32, 7); stand one cell below it.
    let mut rt = Runtime::new(
        data,
        0x010,
        Cell::new(31, 8),
        Direction::Up,
        StepFrames::default(),
    )
    .expect("Piata converts and the spawn cell is legal");

    let mut events = Vec::new();
    for _ in 0..64 {
        events.extend(rt.tick(Input::Direction(Direction::Up)));
        if events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::MapChanged { .. }))
        {
            break;
        }
    }

    assert!(
        events.contains(&RuntimeEvent::MapChanged {
            map: MapId(0x011),
            trigger: WarpTrigger::MapChange,
        }),
        "expected the academy doorway to fire, got {events:?}"
    );
    assert_eq!(rt.map_id(), MapId(0x011));
    // Destination and facing exactly as the cartridge's transition record
    // stores them.
    assert_eq!(rt.state().cell(), Cell::new(31, 19));
    assert_eq!(rt.state().facing(), Direction::Up);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::WarpUnmapped { .. })),
        "no unmapped type-1 cells on the walked path"
    );
}

#[test]
fn every_packed_map_converts_to_an_engine_map() {
    let Some(data) = load() else { return };

    let mut converted = 0usize;
    for (id, record) in data.maps() {
        match psiv_runtime::field_map(record) {
            Ok(_) => converted += 1,
            Err(e) => panic!("map {id:?} failed to convert: {e}"),
        }
    }
    assert!(
        converted >= 350,
        "expected ~359 maps, converted {converted}"
    );
}
