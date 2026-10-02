//! Isolated original-map chest fixtures, derived from an untouched campaign save.
use psiv_core::{Flag, GameState, Inventory, RetailLocation, RetailSave, RetailSlot};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{SaveStore, Session};
use std::path::Path;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let source = RetailSlot::from_bytes(&std::fs::read(&args[1]).unwrap(), 0)
        .unwrap()
        .decode()
        .unwrap();
    let mut game = GameState::from_snapshot(&source.snapshot);
    let (map, x, y, flag) = match args[3].as_str() {
        "full" => {
            let mut items = [125; 40];
            items[0] = 57;
            items[2] = 126;
            *game.inventory_mut() = Inventory::from_slots(items);
            (0x4A, 40, 33, 8)
        }
        "meseta" => {
            *game.inventory_mut() = Inventory::from_slots([125; 40]);
            (0x47, 40, 17, 31)
        }
        "white" => {
            *game.inventory_mut() = Inventory::new();
            (0xA6, 46, 18, 38)
        }
        "normal" => {
            *game.inventory_mut() = Inventory::new();
            (0x47, 17, 18, 32)
        }
        _ => panic!("expected normal, full, meseta or white"),
    };
    game.clear(Flag::chest(flag)).unwrap();
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let output = Path::new(&args[2]);
    // The fixture is a game over a hand-built state; the session owns it from
    // `from_save` on, and writes its slot 1 through the store it was given.
    let session = Session::start(GameData::load(pack).unwrap())
        .with_battles(BattleFiles::load(pack).unwrap())
        .with_saves(SaveStore::new(output))
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: map,
                map_index_2: 0x42,
                char_x: x * 16,
                char_y: y * 16,
            },
        })
        .unwrap();
    println!("{}", session.save_slot(0).unwrap().display());
}
