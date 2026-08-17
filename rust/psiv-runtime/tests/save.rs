//! Runtime-level save/load proof: file I/O plus the loaded-runtime seam.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use psiv_core::battle::{StatPair, StatTriple, Stats};
use psiv_core::{
    CharId, Flag, GameState, MacroCommand, RetailLocation, RetailSave, RetailSlot, StepFrames,
    VehicleRecord,
};
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
        name_bytes: [index, index + 1, index + 2, index + 3, 0xFE, 0],
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
        techniques: [index + 1; 16],
        skills: [index + 2; 8],
        curr_skill_uses: [index + 3; 8],
        max_skill_uses: [index + 4; 8],
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

struct SaveDirectoryEnv {
    _lock: MutexGuard<'static, ()>,
    previous: Option<std::ffi::OsString>,
}

impl SaveDirectoryEnv {
    fn set(path: &Path) -> SaveDirectoryEnv {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let lock = LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("save-directory environment lock");
        let previous = std::env::var_os("PSIV_SAVE_DIR");
        // Rust 2024 makes process-environment mutation explicitly unsafe;
        // the lock scopes this test's temporary override and Drop restores it.
        unsafe { std::env::set_var("PSIV_SAVE_DIR", path) };
        SaveDirectoryEnv {
            _lock: lock,
            previous,
        }
    }

    fn path() -> PathBuf {
        std::env::var_os("PSIV_SAVE_DIR")
            .map(PathBuf::from)
            .expect("PSIV_SAVE_DIR is set to the test directory")
    }
}

impl Drop for SaveDirectoryEnv {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(path) => unsafe { std::env::set_var("PSIV_SAVE_DIR", path) },
            None => unsafe { std::env::remove_var("PSIV_SAVE_DIR") },
        }
    }
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
    game.set_vehicle_index(1);
    game.set_button_mappings_index(2);
    game.set_message_speed(4);
    game.set_battle_speed(3);
    game.macros_mut()[2].commands[1] = MacroCommand {
        character_id: 1,
        command_index: 4,
        ability_id: 0x1F,
        reserved: 0x7E,
    };
    game.vehicles_mut()[0] = VehicleRecord {
        current_hp: 0x0102,
        max_hp: 0x0304,
        skill_mask: 0x03,
        reserved_05: 0x55,
        current_skill_uses: [1, 2, 3, 4, 5, 6, 7, 8],
        max_skill_uses: [8, 7, 6, 5, 4, 3, 2, 1],
        reserved_tail: [0xAA; 10],
    };
    let save = RetailSave {
        snapshot: game.snapshot(),
        location: RetailLocation {
            world_index: 2,
            map_index_2: 0x44,
            map_index: 0x13,
            char_x: 38 * 16,
            char_y: 18 * 16,
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
    let persisted = RetailSlot::from_bytes(
        &std::fs::read(directory.join("slot_1.sram")).expect("read saved slot"),
        0,
    )
    .expect("validate saved slot")
    .decode()
    .expect("decode saved slot");
    assert_eq!(persisted.location.world_index, 2);
    assert_eq!(persisted.location.map_index_2, 0x44);
    assert!(directory.join("slot_1.sram").is_file());
    std::fs::remove_dir_all(directory).expect("remove only this test's temp directory");
}

#[test]
fn title_erase_zeros_only_the_selected_payload_and_preserves_other_slots() {
    let Some(data) = load() else { return };
    let directory = unique_directory();
    let _save_directory = SaveDirectoryEnv::set(&directory);
    let directory = SaveDirectoryEnv::path();
    let save = RetailSave {
        snapshot: GameState::new().snapshot(),
        location: RetailLocation {
            map_index: 0,
            char_x: 16,
            char_y: 16,
            ..RetailLocation::default()
        },
    };
    let runtime =
        Runtime::from_save(data, save, StepFrames::default()).expect("boot erase fixture");
    for slot in 0..3 {
        runtime
            .save_slot(&directory, slot)
            .expect("write erase fixture");
    }
    let before: Vec<Vec<u8>> = (0..3)
        .map(|slot| {
            std::fs::read(directory.join(format!("slot_{}.sram", slot + 1)))
                .expect("read erase fixture")
        })
        .collect();

    Runtime::erase_slot(&directory, 1).expect("erase slot two");

    for slot in [0, 2] {
        let after = std::fs::read(directory.join(format!("slot_{}.sram", slot + 1)))
            .expect("read preserved slot");
        assert_eq!(after, before[slot], "slot {} must survive", slot + 1);
    }
    let erased = std::fs::read(directory.join("slot_2.sram")).expect("read erased slot");
    assert_eq!(
        erased[..psiv_core::RETAIL_HEADER_PHYSICAL_BYTES],
        before[1][..psiv_core::RETAIL_HEADER_PHYSICAL_BYTES]
    );
    assert!(
        erased[psiv_core::RETAIL_HEADER_PHYSICAL_BYTES..]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(matches!(
        RetailSlot::from_bytes(&erased, 1),
        Err(psiv_core::SaveError::ChecksumMismatch { slot: 1, .. })
    ));
    std::fs::remove_dir_all(directory).expect("remove only this test's temp directory");
}
