//! Integration test against a real pack's `battle/` directory.
//!
//! Gated on the pack existing, exactly like `runtime_pack.rs`: the pack is
//! Sega-derived and never committed, so a fresh clone must not fail this.
//! Build one with `python -m psiv_tools pack <rom> runtime-pack/`, or set
//! `PSIV_RUNTIME_PACK` to point somewhere else.

use psiv_data::{BattleFiles, ELEMENT_SLOTS};
use std::path::PathBuf;

fn pack_dir() -> PathBuf {
    match std::env::var_os("PSIV_RUNTIME_PACK") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("runtime-pack"),
    }
}

fn require_battle() -> Option<PathBuf> {
    let dir = pack_dir();
    if dir.join("battle/enemies.json").is_file() {
        return Some(dir);
    }
    eprintln!(
        "skipping: no battle/ under {}. Build a pack with \
         `python -m psiv_tools pack <rom> runtime-pack/`, or set PSIV_RUNTIME_PACK.",
        dir.display()
    );
    None
}

#[test]
fn the_whole_battle_directory_loads_and_validates() {
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");

    // The census the packer prints on the way out.
    assert_eq!(files.enemies.enemies.len(), 153);
    assert_eq!(files.formations.formations.len(), 504);
    assert_eq!(files.formations.boss_formations.len(), 27);
    assert_eq!(files.levels.characters.len(), 11);
    assert_eq!(
        files
            .levels
            .characters
            .iter()
            .map(|c| c.levels.len())
            .sum::<usize>(),
        937
    );
    assert_eq!(files.abilities.all().count(), 366);
}

#[test]
fn the_records_the_oracle_logged_are_the_records_the_pack_carries() {
    // Tape 07 and 09 read these seven stats out of live RAM and matched them
    // against `generated/enemies.json`; this closes the loop through the pack.
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");

    let zoran = files
        .enemies
        .enemies
        .iter()
        .find(|e| e.id == 10)
        .expect("enemy 10");
    assert_eq!(zoran.symbol, "ZoranBult");
    assert_eq!(zoran.hp, 25);
    assert_eq!(zoran.stats.attack, 16);
    assert_eq!(zoran.stats.defense, 2);
    assert_eq!(zoran.stats.agility, 6);
    assert_eq!(zoran.stats.strength, 18);
    assert_eq!(zoran.stats.mental, 4);
    assert_eq!(zoran.stats.dexterity, 8);
    assert_eq!(zoran.rewards.experience, 12, "24 / 3 = 8 each in tape 07");
    assert_eq!(zoran.rewards.meseta, 3);
    assert_eq!(zoran.attack.element.id, 1, "physical");
    assert!(
        zoran.ai.regular_ability_ids.iter().all(|id| *id == 0),
        "a plain-attack enemy, which is why Tier 1 can fight it"
    );

    let xana = files
        .enemies
        .enemies
        .iter()
        .find(|e| e.id == 9)
        .expect("enemy 9");
    assert_eq!(xana.symbol, "Xanafalgue");
    assert_eq!(xana.hp, 16);
    assert_eq!(xana.stats.attack, 13);
    assert_eq!(xana.stats.defense, 0);
    assert_eq!(xana.stats.agility, 5);
    assert_eq!(xana.rewards.experience, 9, "9 + 12 = 21 in tape 09");
    assert_eq!(xana.rewards.meseta, 2);
}

#[test]
fn every_element_property_value_is_one_the_damage_pipeline_expects() {
    // The factor multiplies damage and then divides by four, so a value the
    // scout did not see would change the arithmetic's reachable range.
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");

    let mut seen: Vec<u8> = files
        .enemies
        .enemies
        .iter()
        .flat_map(|e| e.properties.values().map(|p| p.value))
        .collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen, vec![0, 1, 2, 3, 4]);
}

#[test]
fn the_formation_the_scouts_worked_example_uses_is_what_it_says_it_is() {
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");

    let first = &files.formations.formations[0];
    assert_eq!(first.id, Some(0));
    assert_eq!(first.ambush_chance, 0x10, "25% ambush at agility 7");
    assert_eq!(first.run_chance, 0);
    assert!(first.can_run);
    assert_eq!(first.drop_rate, 8, "8 in 128");
    assert_eq!(first.drop_item, Some(0x80), "an Antidote");
    assert_eq!(first.enemies.len(), 2);
    assert!(first.enemies.iter().all(|e| e.enemy_id == 1), "MonsterFly");
}

#[test]
fn the_one_formation_whose_count_lies_is_the_one_the_notes_name() {
    // SOURCE_NOTES: formation 0x177 declares four enemies and lists three.
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");

    let liars: Vec<String> = files
        .formations
        .formations
        .iter()
        .chain(&files.formations.boss_formations)
        .filter(|f| !f.count_matches_entries)
        .map(psiv_data::Formation::key)
        .collect();
    assert_eq!(liars, vec!["formation 375"], "0x177, and only that one");
}

#[test]
fn an_encounter_group_holds_the_thirty_two_entries_the_roll_masks_for() {
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");
    let groups = files
        .formations
        .encounter_groups
        .as_ref()
        .expect("the pack carries encounter groups");
    assert_eq!(
        groups.entries_per_group, 32,
        "the `andi.w #$1F` in the roll"
    );
    for group in &groups.groups {
        assert_eq!(group.formation_ids.len(), 32, "group {}", group.group);
    }
}

#[test]
fn the_level_tables_start_where_the_pointer_table_says() {
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");

    let chaz = &files.levels.characters[0];
    assert_eq!(chaz.character, "Chaz");
    assert_eq!(chaz.starting_level, 1);
    assert_eq!(chaz.levels[0].level, 2);
    assert_eq!(chaz.levels[0].experience_required, 21);
    assert_eq!(chaz.levels[0].hp, 31);
    assert_eq!(chaz.levels[0].stats.strength, 9, "attack 18 becomes 19");

    let alys = &files.levels.characters[1];
    assert_eq!(alys.character, "Alys");
    assert_eq!(alys.starting_level, 7, "her table is indexed from 7, not 1");
    assert_eq!(alys.levels[0].level, 8);

    // Every table's first record is one past its starting level, which is the
    // whole reason `next_after` subtracts before indexing.
    for character in &files.levels.characters {
        assert_eq!(
            character.levels[0].level,
            character.starting_level + 1,
            "{}",
            character.character
        );
    }
}

#[test]
fn the_dispatch_table_is_the_forty_four_entries_at_0x0061be() {
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");
    assert_eq!(files.abilities.effects.count, 44);
    assert_eq!(files.abilities.effects.table, "0x0061BE");
    assert!(
        !files.abilities.effects.bounds_checked,
        "TRAP #2 dispatches with no bound"
    );
}

#[test]
fn black_wave_is_the_only_record_the_load_would_have_refused() {
    // `BattleFiles::load` rejects an out-of-range effect id, so a retail pack
    // must not contain one -- which it does not, because `Zio3`'s BLACK WAVE
    // is flagged and the packer's own census names it. Parsing the file
    // directly is how that gets checked without tripping the validator.
    let Some(dir) = require_battle() else { return };
    let text = std::fs::read_to_string(dir.join("battle/abilities.json")).expect("readable");
    let file: psiv_data::AbilitiesFile = serde_json::from_str(&text).expect("parses");

    let offenders: Vec<String> = file.rejected().map(|a| a.identity()).collect();
    assert_eq!(
        offenders,
        vec!["enemy_skills 112 (BLACK WAVE)"],
        "one record"
    );
    // The pack still loads: refusing it would refuse every retail pack.
    // `usable()` is where the rejection lives.
    let files = BattleFiles::load(&dir).expect("battle/ loads");
    assert_eq!(files.abilities.rejected().count(), 1);
    assert_eq!(
        files.abilities.usable().count(),
        files.abilities.all().count() - 1
    );

    // And it is genuinely fielded, which an earlier note got wrong: Zio3 is
    // boss formation 4, the scripted Zio fight.
    assert_eq!(
        files.fielded_rejected_abilities(),
        vec![("Zio3", 112)],
        "reachable in play, not merely present in the data"
    );
}

// ---------------------------------------------------------------------------
// The seating path: characters.json + equipment.json
// ---------------------------------------------------------------------------

#[test]
fn the_party_files_load_and_agree_with_each_other() {
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");
    assert_eq!(files.characters.characters.len(), 11);
    assert_eq!(files.equipment.items.len(), 160);
    assert_eq!(files.equipment.types.len(), 10);

    // Validation already proved every slot resolves; this proves the lookup
    // the bridge will use finds the same records.
    for character in &files.characters.characters {
        for (slot, filled) in character.equipment.slots() {
            let Some(filled) = filled else { continue };
            let item = files
                .equipment
                .item(filled.item_id)
                .unwrap_or_else(|| panic!("{} {slot}", character.symbol));
            assert_eq!(item.symbol, filled.symbol);
            assert_eq!(item.kind.id, filled.kind, "the repeated type byte agrees");
        }
    }
}

#[test]
fn the_type_table_is_the_one_the_engine_hardcodes() {
    // `psiv_core::battle::ItemKind` encodes this table. The two cannot be
    // compared directly across the crate boundary, so both are pinned against
    // the pack and meet in the middle.
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");

    /// `(type byte, name, slot, two-handed, weapon, multi-target, element role)`
    type Row = (
        u8,
        &'static str,
        Option<&'static str>,
        bool,
        bool,
        bool,
        Option<&'static str>,
    );

    let expected: [Row; 10] = [
        (
            1,
            "one_handed_single_target_weapon",
            Some("right_hand"),
            false,
            true,
            false,
            Some("attack_element"),
        ),
        (
            2,
            "one_handed_multi_target_weapon",
            Some("right_hand"),
            false,
            true,
            true,
            Some("attack_element"),
        ),
        (
            3,
            "two_handed_single_target_weapon",
            Some("right_hand"),
            true,
            true,
            false,
            Some("attack_element"),
        ),
        (
            4,
            "two_handed_multi_target_weapon",
            Some("right_hand"),
            true,
            true,
            true,
            Some("attack_element"),
        ),
        (
            5,
            "shield",
            Some("left_hand"),
            false,
            false,
            false,
            Some("resistance_granted"),
        ),
        (
            6,
            "headwear_or_ring",
            Some("head"),
            false,
            false,
            false,
            Some("resistance_granted"),
        ),
        (
            7,
            "body_equipment",
            Some("body"),
            false,
            false,
            false,
            Some("resistance_granted"),
        ),
        (8, "disposable_item", None, false, false, false, None),
        (9, "plot_item", None, false, false, false, None),
        (10, "field_only_item", None, false, false, false, None),
    ];
    for (byte, name, slot, two_handed, is_weapon, multi, role) in expected {
        let entry = files.equipment.kind(byte).expect("a known type");
        assert_eq!(entry.name, name, "type {byte}");
        assert_eq!(entry.slot.as_deref(), slot, "type {byte} slot");
        assert_eq!(entry.two_handed, two_handed, "type {byte}");
        assert_eq!(entry.is_weapon, is_weapon, "type {byte}");
        assert_eq!(entry.multi_target, multi, "type {byte}");
        assert_eq!(
            entry.element_role.as_deref(),
            role,
            "type {byte} element role"
        );
        assert_eq!(entry.equippable, slot.is_some(), "type {byte}");
    }
}

#[test]
fn every_item_agrees_with_its_own_type_entry() {
    // The per-item convenience flags are derived from the type table, so a
    // disagreement would mean the pack contradicts itself.
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");
    for item in &files.equipment.items {
        let entry = files
            .equipment
            .kind(item.kind.id)
            .unwrap_or_else(|| panic!("{} has type {}", item.symbol, item.kind.id));
        assert_eq!(item.is_weapon, entry.is_weapon, "{}", item.symbol);
        assert_eq!(item.multi_target, entry.multi_target, "{}", item.symbol);
        assert_eq!(item.two_handed, entry.two_handed, "{}", item.symbol);
        assert_eq!(item.kind.name, entry.name, "{}", item.symbol);
        assert_eq!(
            item.element.role.as_deref(),
            entry.element_role.as_deref(),
            "{} element role",
            item.symbol
        );
    }
}

#[test]
fn the_element_byte_really_does_have_two_jobs() {
    // The dual role docs/source-notes/disassembly-discrepancies.md records: the same byte is an attack element on
    // a weapon and a granted resistance on armour.
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");

    let hunt_knife = files.equipment.item(2).expect("Hunt-Knife");
    assert_eq!(hunt_knife.element.id, 1);
    assert_eq!(hunt_knife.element.role.as_deref(), Some("attack_element"));

    let ceramic = files.equipment.item(38).expect("Ceramic Mail");
    assert_eq!(ceramic.element.id, 3, "fire");
    assert_eq!(
        ceramic.element.role.as_deref(),
        Some("resistance_granted"),
        "the same byte, read the other way"
    );

    // Neither role exists on something that cannot be equipped.
    let unequippable = files
        .equipment
        .items
        .iter()
        .find(|item| item.kind.id >= 8)
        .expect("retail has plenty");
    assert_eq!(unequippable.element.role, None);
}

#[test]
fn the_conformance_vectors_are_the_numbers_the_engine_reproduces() {
    // The other half of `psiv_core::battle::party_tests`. That suite seats each
    // record and asserts it derives these; this one asserts these are what the
    // live pack holds. Together they close the loop across the crate boundary.
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");

    let by_id = |id: u8| {
        files
            .characters
            .characters
            .iter()
            .find(|c| c.character_id == id)
            .unwrap_or_else(|| panic!("character {id}"))
    };

    // Scout §12's two hand-worked derivations, now cartridge-derived.
    let chaz = by_id(0);
    assert_eq!(chaz.symbol, "Chaz");
    assert_eq!(chaz.stats.strength, 8);
    assert_eq!(chaz.equipment.item_ids(), [2, 2, 5, 4]);
    assert_eq!(chaz.initialized.atk_pow, 18, "8 + two Hunt-Knives at 5");
    assert_eq!(chaz.initialized.dfs_pow, 10, "7 + helm 1 + cloth 2");
    assert_eq!(chaz.initialized.magic_dfs, 6);

    let alys = by_id(1);
    assert_eq!(
        alys.equipment.item_ids(),
        [3, 0, 6, 4],
        "an empty left hand"
    );
    assert_eq!(alys.initialized.atk_pow, 13);
    assert_eq!(alys.initialized.dfs_pow, 18);

    // Every character's `mod` stats and the three derived words are present.
    for character in &files.characters.characters {
        for stat in ["strength", "mental", "agility", "dexterity"] {
            assert!(
                character.initialized.stats.contains_key(stat),
                "{} {stat}",
                character.symbol
            );
        }
        assert_eq!(
            character.initialized.element_props.len(),
            ELEMENT_SLOTS,
            "{}",
            character.symbol
        );
    }
}

#[test]
fn three_characters_start_with_a_non_physical_weapon() {
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");
    let energy: Vec<&str> = files
        .characters
        .characters
        .iter()
        .filter(|c| c.initialized.weapon_elements.right_hand.id == 2)
        .map(|c| c.symbol.as_str())
        .collect();
    assert_eq!(energy, vec!["Wren", "Kyra", "Seth"]);

    // And an empty hand caches element 0, not the other hand's.
    let alys = files
        .characters
        .characters
        .iter()
        .find(|c| c.symbol == "Alys")
        .expect("Alys");
    assert_eq!(alys.initialized.weapon_elements.right_hand.id, 1);
    assert_eq!(alys.initialized.weapon_elements.left_hand.id, 0);
}

#[test]
fn retail_equipment_carries_genuinely_negative_bonuses() {
    // Which is why the byte adder's lack of sign extension is a live
    // difference rather than a curiosity. The census names the ranges; this
    // finds the records.
    let Some(dir) = require_battle() else { return };
    let files = BattleFiles::load(&dir).expect("battle/ loads");
    let negative: Vec<&str> = files
        .equipment
        .items
        .iter()
        .filter(|item| {
            let b = item.bonuses;
            b.strength < 0 || b.mental < 0 || b.agility < 0 || b.dexterity < 0
        })
        .map(|item| item.symbol.as_str())
        .collect();
    assert!(
        !negative.is_empty(),
        "retail has items that make you worse at something"
    );
}
