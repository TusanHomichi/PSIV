//! Runtime-level save/load proof: file I/O plus the loaded-runtime seam.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use psiv_core::battle::{StatPair, StatTriple, Stats};
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::GameData;
use psiv_runtime::Runtime;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn load() -> Option<GameData> {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("runtime pack not present at {PACK}; skipping save test");
        return None;
    }
    Some(GameData::load(Path::new(PACK)).expect("pack loads and validates"))
}

fn stats(index: u8) -> Stats {
    Stats {
        profession: 1,
        level: 5 + u16::from(index),
        experience: 800 + u32::from(index),
        curr_hp: 20 + u16::from(index),
        max_hp: 40 + u16::from(index),
        curr_tp: 8 + u16::from(index),
        max_tp: 16 + u16::from(index),
        status: 0,
        strength: StatTriple::uniform(10 + index),
        mental: StatTriple::uniform(8 + index),
        agility: StatTriple::uniform(9 + index),
        dexterity: StatTriple::uniform(7 + index),
        attack: StatPair::uniform(30 + u16::from(index)),
        defence: StatPair::uniform(24 + u16::from(index)),
        mental_defence: StatPair::uniform(18 + u16::from(index)),
        element_props: [0; 14],
        element_shadow: [0; 14],
        weapon_elements: [0; 2],
        equipment: [0; 4],
        physical_prop_save: 0,
        enemy_id: 0,
        gain_exp_flag: index == 0,
    }
}

fn unique_directory() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("psiv-save-test-{}-{nanos}", std::process::id()))
}

#[test]
fn runtime_save_file_round_trips_a_mid_progress_game_state() {
    let Some(data) = load() else { return };
    let mut game = GameState::new();
    game.set(Flag::event(0x12)).unwrap();
    game.set(Flag::chest(0x18)).unwrap();
    game.set(Flag::temp(0x24)).unwrap();
    game.set(Flag::town(0x2A)).unwrap();
    game.inventory_mut().add(0x7D).unwrap();
    game.inventory_mut().add(0x22).unwrap();
    game.add_money(4321);
    game.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    for index in 0..11 {
        game.roster_mut().seat(CharId(index), stats(index)).unwrap();
    }
    let chaz = game.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.curr_hp = 3;
    chaz.status = 1;
    chaz.equipment = [0x21, 0x22, 0x23, 0x24];
    let save = RetailSave {
        snapshot: game.snapshot(),
        location: RetailLocation {
            map_index: 0x13,
            char_x: 38 * 16,
            char_y: 18 * 16,
            ..RetailLocation::default()
        },
    };
    let expected = GameState::from_snapshot(&save.snapshot);
    let directory = unique_directory();
    let runtime = Runtime::from_save(data.clone(), save, StepFrames::default()).expect("boot save");
    runtime
        .save_slot(&directory, 0)
        .expect("write retail-shaped slot");
    let reloaded = Runtime::load_slot(data, &directory, 0, StepFrames::default())
        .expect("load retail-shaped slot");

    assert_eq!(runtime.game(), &expected);
    assert_eq!(reloaded.game(), runtime.game());
    assert_eq!(reloaded.map_id(), runtime.map_id());
    assert_eq!(reloaded.state().cell(), runtime.state().cell());
    assert!(directory.join("slot_1.sram").is_file());
    std::fs::remove_dir_all(directory).expect("remove only this test's temp directory");
}
