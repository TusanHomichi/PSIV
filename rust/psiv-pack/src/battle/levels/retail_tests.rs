//! the retired Python battle_pack suite's level checks (#549): `battle/levels.json` as
//! [`build`](super::build) writes it from the local retail ROM.

use psiv_data::LevelsFile;

use crate::battle::tests::retail;

#[test]
fn the_progression_covers_eleven_characters() {
    let Some(retail) = retail() else { return };
    let file: LevelsFile = retail.typed("levels");
    assert_eq!((file.character_count, file.total_records), (11, 937));
    assert_eq!(file.characters.len(), 11);
    let raw = &retail.file("levels")["characters"];
    let records: u64 = raw
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["record_count"].as_u64().unwrap())
        .sum();
    assert_eq!(records, 937);
    let listed: usize = file.characters.iter().map(|c| c.levels.len()).sum();
    assert_eq!(listed, 937);
    let chaz = &file.characters[0];
    assert_eq!((chaz.character.as_str(), chaz.starting_level), ("Chaz", 1));
    let level = &chaz.levels[0];
    assert_eq!((level.level, level.experience_required), (2, 21));
    assert_eq!((level.hp, level.tp), (31, 13));
    assert_eq!(level.skill_uses.len(), 8);
}

#[test]
fn each_characters_levels_run_from_its_starting_level_to_the_highest() {
    let Some(retail) = retail() else { return };
    let file: LevelsFile = retail.typed("levels");
    let highest = retail.file("levels")["census"]["highest_level"]
        .as_u64()
        .unwrap();
    assert_eq!(highest, 99);
    for (id, character) in file.characters.iter().enumerate() {
        assert_eq!(usize::from(character.character_id), id);
        let levels: Vec<u16> = character.levels.iter().map(|l| l.level).collect();
        // Consecutive, from the level after the one the character joins at.
        let expected: Vec<u16> = (character.starting_level + 1..=highest as u16).collect();
        assert_eq!(levels, expected, "{}", character.character);
    }
}
