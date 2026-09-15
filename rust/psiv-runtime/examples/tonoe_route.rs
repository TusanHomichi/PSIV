//! Connected route scout from the native Holt save to Rune and Tonoe.
mod support;
use psiv_core::{Cell, Direction, Flag, Input, StepFrames};
use psiv_data::{BattleFiles, DialogueSet, GameData};
use psiv_runtime::Runtime;
use std::path::Path;
use support::Walk;

fn main() {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let save_dir =
        std::env::var("PSIV_ROUTE_CONTINUE_DIR").expect("native Holt save directory required");
    let mut rt = Runtime::load_slot(
        GameData::load(pack).unwrap(),
        Path::new(&save_dir),
        0,
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&BattleFiles::load(pack).unwrap())
        .unwrap();
    let mut route = Walk {
        rt,
        dialogue: DialogueSet::load(pack).unwrap(),
        ticks: 0,
        battles: 0,
        heal_in_battle: true,
    };
    route.checkpoint("continued native Holt save");
    assert!(route.rt.game().is_set(Flag::event(16)));
    route.walk_to(Cell::new(82, 158));
    assert_eq!(route.rt.map_id().0, 0x40);
    route.checkpoint("Molcum");
    route.walk_to(Cell::new(30, 22));
    route.tick(Input::Direction(Direction::Up));
    route.tick(Input::Neutral);
    route.tick(Input::Action);
    route.settle();
    assert!(route.rt.game().is_set(Flag::event(17)));
    route.checkpoint("Rune joined");
    route.walk_to(Cell::new(30, 56));
    assert_eq!(route.rt.map_id().0, 0);
    route.walk_to(Cell::new(128, 121));
    assert_eq!(route.rt.map_id().0, 0xD8);
    assert!(route.rt.game().is_set(Flag::event(0x13)));
    route.checkpoint("Rune clears Valley Maze entrance");
    for (target, destination) in [
        (Cell::new(31, 31), 0x9B),
        (Cell::new(16, 11), 0x9D),
        (Cell::new(16, 15), 0x9F),
        (Cell::new(18, 15), 0xA1),
        (Cell::new(32, 15), 0xD9),
        (Cell::new(31, 36), 0),
    ] {
        route.walk_to(target);
        assert_eq!(route.rt.map_id().0, destination);
        route.checkpoint("Valley Maze");
    }
    route.walk_to(Cell::new(150, 88));
    assert_eq!(route.rt.map_id().0, 0x41);
    route.checkpoint("Tonoe");
    route.walk_to(Cell::new(45, 21));
    assert_eq!(route.rt.map_id().0, 0x43);
    route.walk_to(Cell::new(30, 32));
    route.tick(Input::Direction(Direction::Up));
    route.tick(Input::Neutral);
    route.tick(Input::Action);
    route.settle();
    route.checkpoint("Dorin");
    if let Some(dir) = std::env::var_os("PSIV_ROUTE_SAVE_DIR") {
        route.rt.save_slot(Path::new(&dir), 0).unwrap();
    }
}
