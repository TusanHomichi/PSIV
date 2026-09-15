//! Native START through the opening, with real map loads and no state resets.

use std::path::Path;

use psiv_core::{CharId, Direction, Flag, Input, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn native_opening_reaches_retail_first_control_with_its_initial_state_intact() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let data = GameData::load(pack).expect("pack loads");
    let expected = data.manifest().game_start.clone().expect("first control");
    let event = data.new_game().expect("title init").event_index;
    let mut runtime = Runtime::new_game(data, StepFrames::default()).expect("START");
    runtime
        .enable_battles(&BattleFiles::load(pack).expect("battle pack"))
        .expect("battles enabled");
    assert!(runtime.start_event(event));
    let mut saw_chaz_alone = false;
    for _ in 0..8_000 {
        for event in runtime.tick(Input::Neutral) {
            match event {
                RuntimeEvent::SceneDialogue { entry } => {
                    saw_chaz_alone |= entry == 0x6D;
                    runtime.dialogue_closed();
                }
                RuntimeEvent::SceneDialogueResume => runtime.dialogue_closed(),
                RuntimeEvent::SceneFaulted { .. }
                | RuntimeEvent::UnpackedTarget { .. }
                | RuntimeEvent::SceneMissing { .. }
                | RuntimeEvent::SceneBattleFailed { .. } => panic!("opening failed: {event:?}"),
                _ => {}
            }
        }
        if !runtime.scene_active() && runtime.game().is_set(Flag::event(21)) {
            break;
        }
    }
    assert!(
        saw_chaz_alone,
        "Chaz's first line must happen before the player moves"
    );
    assert!(!runtime.scene_active(), "opening releases input");
    assert_eq!(runtime.map_id().0, expected.map.id);
    assert_eq!(u32::from(runtime.state().cell().x), expected.x_cell);
    assert_eq!(u32::from(runtime.state().cell().y), expected.y_cell);
    assert_eq!(runtime.game().party_members(), vec![CharId(0)]);
    assert_eq!(runtime.game().money(), 500);
    let snapshot = runtime.game().snapshot();
    // Fresh oracle tape 01, frame 6456: the full banks, not selected bits.
    let mut flags = [0u8; 64];
    for id in expected
        .event_flags_set
        .iter()
        .chain(&expected.extended_event_flags_set)
    {
        flags[usize::from(*id / 8)] |= 0x80 >> (id & 7);
    }
    assert_eq!(snapshot.event_flags, flags);
    assert_eq!(snapshot.temp_flags, [0; 32]);
    assert_eq!(&snapshot.town_flags[..4], &[0x80, 0, 0x80, 0x40]);
    assert!(snapshot.town_flags[4..].iter().all(|byte| *byte == 0));
    assert!(snapshot.inventory.iter().all(|item| *item == 0));
    assert!(snapshot.characters.iter().all(Option::is_some));
    assert_eq!(snapshot.characters[0].as_ref().unwrap().curr_hp, 25);
    assert_eq!(snapshot.message_speed, 2);
    assert_eq!(snapshot.battle_speed, 2);

    // The next directional input moves the player rather than starting a
    // delayed opening line. Its position must remain writable after a load.
    let start = runtime.state().cell();
    for _ in 0..8 {
        runtime.tick(Input::Direction(Direction::Up));
    }
    assert_ne!(
        runtime.state().cell(),
        start,
        "field movement works after START"
    );
    assert!(!runtime.scene_active());

    let directory = std::env::temp_dir().join(format!("psiv-new-game-save-{}", std::process::id()));
    let path = runtime
        .save_slot(&directory, 0)
        .expect("save first-control state");
    let mut restored =
        Runtime::load_slot(runtime.data().clone(), &directory, 0, StepFrames::default())
            .expect("continue the saved game");
    restored
        .enable_battles(&BattleFiles::load(pack).expect("battle pack"))
        .expect("continue arms battles");
    assert_eq!(
        restored.game().snapshot(),
        runtime.game().snapshot(),
        "CONTINUE must not reseed saved state"
    );
    assert_eq!(restored.state().cell(), runtime.state().cell());
    std::fs::remove_file(path).expect("remove test save");
    std::fs::remove_dir(directory).expect("remove test save directory");
}
