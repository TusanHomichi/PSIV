//! One new game through the Academy job, using ordinary runtime commands.
mod support;
use psiv_core::{Cell, Direction, Flag, Input, StepFrames};
use psiv_data::{BattleFiles, DialogueSet, GameData};
use psiv_runtime::Runtime;
use std::path::Path;
use support::Walk;

fn main() {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let data = GameData::load(pack).unwrap();
    let opening = data.new_game().unwrap().event_index;
    let mut rt = Runtime::new_game(data, StepFrames::default()).unwrap();
    rt.enable_battles(&BattleFiles::load(pack).unwrap())
        .unwrap();
    assert!(rt.start_event(opening));
    let mut route = Walk {
        rt,
        dialogue: DialogueSet::load(pack).unwrap(),
        ticks: 0,
        battles: 0,
        heal_in_battle: false,
    };
    route.settle();
    route.settle(); // The first-control trigger runs before movement.
    route.checkpoint("START opening");
    route.walk_to(Cell::new(38, 17));
    assert!(route.rt.game().is_set(Flag::event(8)));
    route.checkpoint("Alys joined");
    route.walk_to(Cell::new(31, 10));
    assert_eq!(route.rt.map_id().0, 0x14);
    route.walk_to(Cell::new(15, 16));
    route.tick(Input::Direction(Direction::Up));
    route.tick(Input::Neutral);
    route.tick(Input::Action);
    route.settle();
    assert!(route.rt.game().is_set(Flag::event(9)));
    route.checkpoint("principal briefing");
    route.walk_to(Cell::new(15, 21));
    assert_eq!(route.rt.map_id().0, 0x13);
    route.settle();
    route.checkpoint("back from principal");
    route.walk_to(Cell::new(16, 19));
    assert_eq!(route.rt.map_id().0, 0x11);
    route.walk_to(Cell::new(30, 8));
    assert_eq!(route.rt.map_id().0, 0x12);
    route.walk_to(Cell::new(13, 18));
    route.tick(Input::Direction(Direction::Up));
    route.tick(Input::Neutral);
    route.tick(Input::Action);
    route.settle();
    assert!(route.rt.game().is_set(Flag::event(10)));
    route.checkpoint("Hahn joined");
    route.walk_to(Cell::new(18, 13));
    assert_eq!(route.rt.map_id().0, 0x15);
    route.checkpoint("basement entrance");
    route.walk_to(Cell::new(32, 9));
    assert_eq!(route.rt.map_id().0, 0x16);
    route.checkpoint("basement B1");
    route.walk_to(Cell::new(14, 19));
    assert_eq!(route.rt.map_id().0, 0x17);
    route.checkpoint("basement B2");
    route.walk_to(Cell::new(15, 11));
    route.tick(Input::Direction(Direction::Up));
    route.tick(Input::Neutral);
    route.tick(Input::Action);
    route.settle();
    route.checkpoint("Igglanova defeated");
    assert!(route.rt.game().is_set(Flag::event(11)));
    route.settle();
    route.checkpoint("after Igglanova conversation");
    assert!(route.rt.game().is_set(Flag::event(15)));
    route.walk_to(Cell::new(16, 19));
    assert_eq!(route.rt.map_id().0, 0x16);
    route.walk_to(Cell::new(48, 9));
    assert_eq!(route.rt.map_id().0, 0x15);
    route.walk_to(Cell::new(48, 9));
    assert_eq!(route.rt.map_id().0, 0x12);
    route.walk_to(Cell::new(14, 22));
    assert_eq!(route.rt.map_id().0, 0x11);
    route.walk_to(Cell::new(16, 19));
    assert_eq!(route.rt.map_id().0, 0x13);
    route.walk_to(Cell::new(31, 10));
    assert_eq!(route.rt.map_id().0, 0x14);
    route.walk_to(Cell::new(15, 16));
    route.tick(Input::Direction(Direction::Up));
    route.tick(Input::Neutral);
    route.tick(Input::Action);
    route.settle();
    assert!(route.rt.game().is_set(Flag::event(12)));
    route.checkpoint("principal paid");
    route.walk_to(Cell::new(15, 21));
    route.walk_to(Cell::new(16, 19));
    route.walk_to(Cell::new(31, 20));
    assert_eq!(route.rt.map_id().0, 0x10);
    route.checkpoint("back upstairs");
    route.walk_to(Cell::new(31, 48));
    assert_eq!(route.rt.map_id().0, 0);
    route.checkpoint("Motavia unlocked");
    if let Some(dir) = std::env::var_os("PSIV_ROUTE_SAVE_DIR") {
        let dir = Path::new(&dir);
        route.rt.save_slot(dir, 0).unwrap();
        let mut resumed =
            Runtime::load_slot(GameData::load(pack).unwrap(), dir, 0, StepFrames::default())
                .unwrap();
        resumed
            .enable_battles(&BattleFiles::load(pack).unwrap())
            .unwrap();
        assert_eq!(resumed.game().snapshot(), route.rt.game().snapshot());
        assert_eq!(resumed.map_id(), route.rt.map_id());
        assert_eq!(resumed.state().cell(), route.rt.state().cell());
        println!("SAVE AND CONTINUE VERIFIED: {} victories", route.battles);
    }
}
