//! Isolated spell verification fixture derived from a native campaign save.
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave, RetailSlot};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{HINAS, RYUKA, SaveStore, Session};
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
    println!("isolated travel fixture: {}", path.display());
}
