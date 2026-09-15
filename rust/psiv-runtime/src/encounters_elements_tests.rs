//! Element ids must survive the JSON names -> fixed cartridge slots bridge.
use super::{battle_data, character_record};
use psiv_core::battle::{ELEMENT_NAMES, PartyMember};
use std::path::Path;

#[test]
fn all_party_and_enemy_resistances_use_cartridge_ids_regardless_of_census_order() {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut files = psiv_data::BattleFiles::load(pack).unwrap();
    let data = battle_data(&files).unwrap();
    // Regression: alphabetical slot 11 was mechanical (immune), making
    // Earth fail against ZoranBult regardless of the roll.
    assert_eq!(data.enemy(10).unwrap().properties[10], 2);
    assert_eq!(data.enemy(10).unwrap().properties[11], 0);
    for enemy in &files.enemies.enemies {
        let native = data.enemy(enemy.id).unwrap();
        for (slot, name) in ELEMENT_NAMES.iter().enumerate() {
            assert_eq!(
                native.properties[slot],
                enemy.properties[*name].value,
                "enemy {} element {} ({name})",
                enemy.id,
                slot + 1
            );
        }
    }
    for character in &files.characters.characters {
        let native = character_record(character, &files.enemies.properties).unwrap();
        for (slot, name) in ELEMENT_NAMES.iter().enumerate() {
            assert_eq!(
                native.properties[slot],
                character.properties[*name].value,
                "character {} element {} ({name})",
                character.character_id,
                slot + 1
            );
        }
        let stats = PartyMember::seat(&native, &data).unwrap().stats;
        for (slot, name) in ELEMENT_NAMES.iter().enumerate() {
            assert_eq!(
                stats.element_factor((slot + 1) as u8),
                Some(character.initialized.element_props[*name].value),
                "equipped character {} element {} ({name})",
                character.character_id,
                slot + 1
            );
        }
    }
    files.enemies.properties.reverse();
    let reversed = battle_data(&files).unwrap();
    assert_eq!(reversed.enemy(10).unwrap(), data.enemy(10).unwrap());
    // Duplicate/missing names must fail rather than silently reassign slots.
    files.enemies.properties[0] = files.enemies.properties[1].clone();
    assert!(battle_data(&files).is_err());
}
