//! Five original starting records, deliberately reordered to expose UI seat bugs.
use psiv_core::{CharId, GameState, RetailLocation, RetailSave};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{SaveStore, Session};
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
    let data = GameData::load(pack).unwrap();
    let files = BattleFiles::load(pack).unwrap();
    // The records are the retail boot's own (`field`), reordered in the party.
    let initial = Session::start(data.clone())
        .with_battles(files.clone())
        .field()
        .unwrap();
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(8)),
        Some(CharId(0)),
        Some(CharId(2)),
        Some(CharId(4)),
        Some(CharId(3)),
    ]);
    let session = Session::start(data)
        .with_battles(files)
        .with_saves(SaveStore::new(output))
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: 0x47,
                map_index_2: 0x42,
                char_x: 480,
                char_y: 720,
            },
        })
        .unwrap();
    println!(
        "isolated five-character original-stat fixture: {}",
        session.save_slot(0).unwrap().display()
    );
}
