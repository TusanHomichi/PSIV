//! Isolated spell verification fixture derived from a native campaign save.
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave, RetailSlot, StepFrames};
use psiv_data::GameData;
use psiv_runtime::{HINAS, RYUKA, Runtime};
use std::path::Path;

fn main() {
    let source = std::env::args().nth(1).expect("source slot file");
    let output = std::env::args().nth(2).expect("isolated output directory");
    let save = RetailSlot::from_bytes(&std::fs::read(source).unwrap(), 0)
        .unwrap()
        .decode()
        .unwrap();
    let mut game = GameState::from_snapshot(&save.snapshot);
    let chaz = game.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.techniques = [RYUKA, HINAS, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    chaz.curr_tp = 50;
    chaz.max_tp = 50;
    chaz.status = 0;
    game.set(Flag::town(1)).unwrap();
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let rt = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index: 0x47,
                map_index_2: 0x42,
                char_x: 30 * 16,
                char_y: 45 * 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    let path = rt.save_slot(Path::new(&output), 0).unwrap();
    println!("isolated travel fixture: {}", path.display());
}
