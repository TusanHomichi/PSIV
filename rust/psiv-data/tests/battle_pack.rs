//! Integration test against a real pack's `battle/` directory.
//!
//! Gated on the pack existing, exactly like `runtime_pack.rs`: the pack is
//! Sega-derived and never committed, so a fresh clone must not fail this.
//! Build one with `python -m psiv_tools pack <rom> runtime-pack/`, or set
//! `PSIV_RUNTIME_PACK` to point somewhere else.

use psiv_data::BattleFiles;
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
        "one record, on an enemy no formation uses"
    );
    // The pack still loads: refusing it over a record no formation reaches
    // would refuse every retail pack. `usable()` is where the rejection lives.
    let files = BattleFiles::load(&dir).expect("battle/ loads");
    assert_eq!(files.abilities.rejected().count(), 1);
    assert_eq!(
        files.abilities.usable().count(),
        files.abilities.all().count() - 1
    );
}
