//! Isolated original-map chest fixtures, derived from an untouched campaign save.
use psiv_core::{Flag, GameState, Inventory, RetailLocation, RetailSave, RetailSlot, StepFrames};
use psiv_data::GameData;
use psiv_runtime::Runtime;
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
    let runtime = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: map,
                map_index_2: 0x42,
                char_x: x * 16,
                char_y: y * 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    println!(
        "{}",
        runtime.save_slot(Path::new(&args[2]), 0).unwrap().display()
    );
}
