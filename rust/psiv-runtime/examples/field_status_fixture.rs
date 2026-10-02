//! Isolated native verification fixtures; never campaign-progress evidence.
use std::path::Path;

use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{SaveStore, Session};

fn main() {
    let directory = std::env::args().nth(1).expect("output directory required");
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let data = GameData::load(pack).unwrap();
    let files = BattleFiles::load(pack).unwrap();
    // The retail initializer's own state (`new_game`), before the opening's
    // first frame; the fixtures below are edits of it.
    let initial = Session::start(data.clone())
        .with_battles(files.clone())
        .new_game()
        .unwrap();
    for (label, poison) in [("poison", true), ("battle", false)] {
        let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
        game.set(Flag::event(0x0C)).unwrap();
        game.set_party([
            Some(CharId(0)),
            poison.then_some(CharId(2)),
            None,
            None,
            None,
        ]);
        for (id, hp) in [(0, if poison { 2 } else { 1 }), (2, 1)] {
            let stats = game.roster_mut().get_mut(CharId(id)).unwrap();
            stats.curr_hp = hp;
            stats.status = u8::from(poison);
        }
        let slot_dir = Path::new(&directory).join(label);
        let fixture = Session::start(data.clone())
            .with_battles(files.clone())
            .with_saves(SaveStore::new(&slot_dir))
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: 0,
                    char_x: 99 * 16,
                    char_y: 84 * 16,
                },
            })
            .unwrap();
        let path = fixture.save_slot(0).unwrap();
        println!("isolated {label} fixture: {}", path.display());
    }
}
