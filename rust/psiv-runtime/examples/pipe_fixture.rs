//! Isolated pipe verification fixture; campaign roster and original save stay unchanged.
use psiv_core::{Flag, GameState, RetailLocation, RetailSave, RetailSlot};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{ESCAPIPE, SaveStore, Session, TELEPIPE};
use std::path::Path;

fn main() {
    let source = std::env::args().nth(1).expect("source slot file");
    let output = std::env::args().nth(2).expect("isolated output directory");
    let save = RetailSlot::from_bytes(&std::fs::read(source).unwrap(), 0)
        .unwrap()
        .decode()
        .unwrap();
    let mut game = GameState::from_snapshot(&save.snapshot);
    *game.inventory_mut() = psiv_core::Inventory::default();
    for id in [125, TELEPIPE, ESCAPIPE, TELEPIPE, 126] {
        game.inventory_mut().add(id).unwrap();
    }
    game.set(Flag::town(1)).unwrap();
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let session = Session::start(GameData::load(pack).unwrap())
        .with_battles(BattleFiles::load(pack).unwrap())
        .with_saves(SaveStore::new(&output))
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: 0x47,
                map_index_2: 0x42,
                char_x: 30 * 16,
                char_y: 45 * 16,
            },
        })
        .unwrap();
    let path = session.save_slot(0).unwrap();
    println!("isolated pipe fixture: {}", path.display());
}
