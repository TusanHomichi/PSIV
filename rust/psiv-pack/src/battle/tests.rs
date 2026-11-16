//! What [`build`] writes, read back: the pack-level checks of
//! tests/test_battle_pack.py (`TestBattlePack`) and
//! tests/test_battle_records.py (`TestInThePack`), ported for #549. The
//! retail fixture here is shared with the owning modules' own retail tests
//! (enemies, levels, abilities, characters, equipment, formations).
//!
//! Every retail value asserted is an id, a count, an address or a value the
//! Python tests already pinned. Cartridge text is pinned with
//! [`text_pin`](crate::test_support::text_pin), never quoted.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::build;
use crate::test_support::{retail_rom, scratch};

/// The record files `build` writes, by manifest key, with their `kind`.
pub(super) const RECORD_FILES: [(&str, &str, &str); 7] = [
    ("enemies", "battle/enemies.json", "battle_enemies"),
    (
        "enemy_animations",
        "battle/enemy_animations.json",
        "battle_enemy_animations",
    ),
    ("abilities", "battle/abilities.json", "battle_abilities"),
    ("formations", "battle/formations.json", "battle_formations"),
    ("levels", "battle/levels.json", "battle_levels"),
    ("equipment", "battle/equipment.json", "battle_equipment"),
    ("characters", "battle/characters.json", "battle_characters"),
];

/// One emission: the ROM, the `battle` fragment and each record file's text.
pub(super) struct Retail {
    pub rom: Vec<u8>,
    pub fragment: Value,
    pub texts: BTreeMap<&'static str, String>,
    pub files: BTreeMap<&'static str, Value>,
}

impl Retail {
    /// A record file by manifest key.
    pub fn file(&self, key: &str) -> &Value {
        &self.files[key]
    }

    /// A record file read through the type the runtime loads it with.
    pub fn typed<T: DeserializeOwned>(&self, key: &str) -> T {
        serde_json::from_value(self.files[key].clone())
            .unwrap_or_else(|error| panic!("psiv-data refuses {key}: {error}"))
    }
}

/// `build` into a scratch root; the fragment and the record files' text.
fn emit(rom: &crate::Rom, name: &str) -> (Value, BTreeMap<&'static str, String>) {
    let root = scratch(name);
    std::fs::create_dir_all(&root).unwrap();
    let fragment = build(rom, &root);
    let texts = RECORD_FILES
        .iter()
        .map(|(key, path, _)| (*key, std::fs::read_to_string(root.join(path))))
        .collect::<Vec<_>>();
    std::fs::remove_dir_all(&root).unwrap();
    let fragment = Value::from(&fragment.unwrap())["battle"].clone();
    let texts = texts
        .into_iter()
        .map(|(key, text)| (key, text.unwrap()))
        .collect();
    (fragment, texts)
}

/// The battle section built once from the local retail ROM.
pub(super) fn retail() -> Option<&'static Retail> {
    static RETAIL: std::sync::OnceLock<Option<Retail>> = std::sync::OnceLock::new();
    RETAIL
        .get_or_init(|| {
            let rom = retail_rom("battle retail tests")?;
            let (fragment, texts) = emit(&rom, "battle");
            let files = texts
                .iter()
                .map(|(key, text)| (*key, serde_json::from_str(text).unwrap()))
                .collect();
            Some(Retail {
                rom: rom.bytes().to_vec(),
                fragment,
                texts,
                files,
            })
        })
        .as_ref()
}

#[test]
fn every_file_declares_the_pack_version_and_its_kind() {
    let Some(retail) = retail() else { return };
    for (key, path, kind) in RECORD_FILES {
        let file = retail.file(key);
        assert_eq!(file["format_version"], super::FORMAT_VERSION, "{key}");
        assert_eq!(file["kind"], kind, "{key}");
        assert_eq!(retail.fragment["files"][key]["file"], path, "{key}");
        assert!(path.starts_with("battle/"));
    }
    assert_eq!(retail.fragment["directory"], "battle");
}

#[test]
fn the_manifest_counts_are_what_the_cartridge_holds() {
    let Some(retail) = retail() else { return };
    let files = &retail.fragment["files"];
    let counts = [
        (&["enemies", "count"][..], 153),
        (&["formations", "count"], 504),
        (&["formations", "boss_count"], 27),
        (&["formations", "encounter_groups"], 68),
        (&["formations", "inline_count"], 5),
        (&["levels", "characters"], 11),
        (&["levels", "records"], 937),
        (&["abilities", "techniques"], 40),
        (&["abilities", "skills"], 54),
        (&["abilities", "enemy_skills"], 112),
        (&["abilities", "item_effects"], 160),
        (&["characters", "count"], 11),
        (&["equipment", "count"], 160),
        (&["equipment", "equippable"], 132),
        (&["enemy_animations", "count"], 153),
        (&["enemy_animations", "exact_sfx"], 153),
        (&["enemy_animations", "generic_sfx"], 0),
    ];
    for (path, expected) in counts {
        let value = path.iter().fold(files, |value, key| &value[key]);
        assert_eq!(value, expected, "files.{}", path.join("."));
    }
}

#[test]
fn the_manifest_headline_names_the_routines_and_the_census() {
    let Some(retail) = retail() else { return };
    let rules = &retail.fragment["equipment_rules"];
    let routines: Vec<&String> = rules["routines"].as_object().unwrap().keys().collect();
    assert_eq!(
        routines,
        ["attack", "derived_stats", "elements", "equip"]
            .map(String::from)
            .iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(rules["weapon_types"], serde_json::json!([1, 2, 3, 4]));
    assert_eq!(rules["multi_target_types"], serde_json::json!([2, 4]));
    assert_eq!(rules["derived_stats"].as_array().unwrap().len(), 7);
    let census = &retail.fragment["census"];
    assert_eq!(
        census["characters"]["weapon_in_left_hand"],
        serde_json::json!([0, 5])
    );
    assert_eq!(census["equipment"]["equippable"], 132);
    for (key, ..) in RECORD_FILES {
        assert_eq!(census[key], retail.file(key)["census"], "{key}");
    }
    let effects = &retail.fragment["ability_effects"];
    assert_eq!(effects, &retail.file("abilities")["effects"]);
    assert_eq!(effects["bounds_checked"], false);
}

#[test]
fn the_fragment_hashes_are_the_written_files() {
    let Some(retail) = retail() else { return };
    for (key, ..) in RECORD_FILES {
        let digest = crate::png::hex(&Sha256::digest(retail.texts[key].as_bytes()));
        assert_eq!(retail.fragment["files"][key]["sha256"], digest, "{key}");
    }
}

#[test]
fn every_file_is_sorted_two_space_json_ending_in_a_newline() {
    let Some(retail) = retail() else { return };
    for (key, text) in &retail.texts {
        let canonical = serde_json::to_string_pretty(&retail.files[key]).unwrap() + "\n";
        assert!(*text == canonical, "{key} is not sorted, indented JSON");
    }
}

#[test]
fn emitting_twice_writes_the_same_bytes() {
    let Some(retail) = retail() else { return };
    let rom = crate::Rom::for_tests(retail.rom.clone());
    let (fragment, texts) = emit(&rom, "battle-again");
    assert_eq!(fragment, retail.fragment);
    assert_eq!(texts, retail.texts);
}

#[test]
fn every_character_equips_items_the_equipment_file_holds() {
    let Some(retail) = retail() else { return };
    let equipment: psiv_data::EquipmentFile = retail.typed("equipment");
    let characters: psiv_data::CharactersFile = retail.typed("characters");
    for character in &characters.characters {
        for (slot, equipped) in character.equipment.slots() {
            if let Some(equipped) = equipped {
                let item = equipment.item(equipped.item_id).unwrap_or_else(|| {
                    panic!("{} {slot}: no item {}", character.symbol, equipped.item_id)
                });
                assert_eq!(item.symbol, equipped.symbol);
                assert_eq!(item.display_name, equipped.display_name);
                assert_eq!(item.kind.id, equipped.kind);
            }
        }
    }
}

#[test]
fn text_name_counts_line_up_with_emitted_record_tables() {
    let Some(r) = retail() else { return };
    let names = Value::from(&crate::text::extract_names(&r.rom).unwrap());
    for (name_table, file, key) in [
        ("enemy_names", "enemies", "enemies"),
        ("enemy_skill_names", "abilities", "enemy_skills"),
        ("item_names", "equipment", "items"),
        ("technique_names", "abilities", "techniques"),
        ("skill_names", "abilities", "skills"),
        ("character_names", "characters", "characters"),
    ] {
        assert_eq!(
            names[name_table].as_array().unwrap().len(),
            r.file(file)[key].as_array().unwrap().len(),
            "{name_table}"
        );
    }
    // The original metadata table has fifteen combo records (including
    // id zero); no combo record file is emitted by the runtime pack.
    assert_eq!(names["combo_names"].as_array().unwrap().len() + 1, 15);
    assert_eq!(names["combo_names"][0]["id"], 1);
}
