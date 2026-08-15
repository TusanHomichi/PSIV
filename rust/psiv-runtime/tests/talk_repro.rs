use std::path::Path;
use psiv_core::{Cell, Direction, Input, StepFrames};
use psiv_data::GameData;
use psiv_runtime::{Runtime, RuntimeEvent};

#[test]
fn talking_to_an_aligned_npc_in_piata_works() {
    let pack = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
    if !Path::new(pack).join("manifest.json").is_file() { return; }
    let data = GameData::load(Path::new(pack)).unwrap();
    // NPC 5 sits at cell (28,22), cell-aligned. Stand below, face up, press.
    let mut rt = Runtime::new(data, 0x010, Cell::new(28, 23), Direction::Up, StepFrames::default()).unwrap();
    let events = rt.tick(Input::Action);
    eprintln!("events: {events:?}");
    assert!(events.iter().any(|e| matches!(e, RuntimeEvent::Interact { .. })), "expected Interact, got {events:?}");
}
