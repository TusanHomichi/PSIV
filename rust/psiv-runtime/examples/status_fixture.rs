//! Five original starting records, deliberately reordered to expose UI seat bugs.
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;
fn main() {
    let output = std::env::args()
        .nth(1)
        .expect("new fixture output directory");
    let output = Path::new(&output);
    assert!(
        !output.join("slot_1.sram").exists(),
        "output slot must be new"
    );
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x47,
        Cell::new(30, 45),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    initial.enable_battles(&files).unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(8)),
        Some(CharId(0)),
        Some(CharId(2)),
        Some(CharId(4)),
        Some(CharId(3)),
    ]);
    let rt = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: 0x47,
                map_index_2: 0x42,
                char_x: 480,
                char_y: 720,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    println!(
        "isolated five-character original-stat fixture: {}",
        rt.save_slot(output, 0).unwrap().display()
    );
}
