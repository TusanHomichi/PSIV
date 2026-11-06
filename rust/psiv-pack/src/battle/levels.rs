//! `battle/levels.json`: the port of `extract_level_progression` in
//! `psiv_tools/core.py` and `build_levels` in `psiv_tools/battle_pack.py`.
//! `CharLevelTablePtrs` holds a `(starting level .w, table .l)` pair per
//! character; each table runs `99 - starting level` 22-byte records.
//!
//! The file is `psiv_data::LevelsFile`. The pointer table, each table's
//! offset and record count, and the census have no typed home yet and are
//! laid over it.

use std::collections::BTreeSet;

use psiv_data::{Level, LevelStats, LevelTable, LevelsFile, NamedId};

use super::{FORMAT_VERSION, range, serialize};
use crate::PackError;
use crate::json::{Json, overlay};
use crate::png::hex;

/// `TABLES["level_pointers"]`.
const POINTERS: usize = 0x004074;
const POINTER_BYTES: usize = 6;
const RECORD_BYTES: usize = 22;
const LAST_LEVEL: u16 = 99;

const CHARACTERS: [&str; 11] = [
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren", "Raja", "Kyra", "Seth",
];
const TECHNIQUES: [&str; 40] = [
    "Foi", "Gifoi", "Nafoi", "Wat", "Giwat", "Nawat", "Tsu", "Githu", "Nathu", "Zan", "Gizan",
    "Nazan", "Gra", "Gigra", "Nagra", "Megid", "Brose", "Vol", "Savol", "Gelun", "Doran", "Seals",
    "Rimit", "Res", "Gires", "Nares", "Sar", "Gisar", "Nasar", "Shift", "Saner", "Deban", "Feeve",
    "Anti", "Rimpa", "Rever", "Regen", "Arows", "Ryuka", "Hinas",
];
const SKILLS: [&str; 54] = [
    "Crosscut",
    "Rayblade",
    "DblSlash",
    "Flaeli",
    "Flare",
    "Vortex",
    "Astral",
    "Airslash",
    "Disrupt",
    "Hewn",
    "Tandle",
    "Efess",
    "Legeon",
    "Burstroc",
    "Posibolt",
    "Sweeping",
    "Phonon",
    "StFire",
    "Corrsion",
    "Explode",
    "Eliminat",
    "Diem",
    "Spark",
    "Death",
    "Holyword",
    "Dthspell",
    "Negatis",
    "Illusion",
    "Telele",
    "Shadow",
    "Earth",
    "Hijammer",
    "Moonshad",
    "Crash",
    "StasisBm",
    "Bindwa",
    "Mindblst",
    "Barrier",
    "WarCry",
    "Blessing",
    "Warla",
    "Recover",
    "Medice",
    "Miracle",
    "MedicPw",
    "Ataraxia",
    "Vision",
    "Eliminat2",
    "Flaeli2",
    "Hewn2",
    "Tandle2",
    "Spark2",
    "Barrier2",
    "Recover2",
];

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// `_named_id`: 0 is nothing; otherwise a 1-based index into `names`.
fn named(value: u8, names: &[&str]) -> Option<NamedId> {
    (value != 0).then(|| NamedId {
        id: u16::from(value),
        name: names.get(usize::from(value) - 1).map(|&n| n.to_owned()),
    })
}

fn word(rom: &[u8], at: usize) -> Result<u16, PackError> {
    Ok(crate::m68k::w(rom, at)?)
}

fn long(rom: &[u8], at: usize) -> Result<u32, PackError> {
    Ok(crate::m68k::l(rom, at)?)
}

/// One 22-byte record.
fn level(record: &[u8], level: u16) -> Level {
    let be16 = |at: usize| u16::from_be_bytes([record[at], record[at + 1]]);
    Level {
        level,
        experience_required: u32::from_be_bytes(record[0..4].try_into().expect("4 bytes")),
        hp: be16(4),
        tp: be16(6),
        stats: LevelStats {
            strength: record[8],
            mental: record[9],
            agility: record[10],
            dexterity: record[11],
        },
        new_technique: named(record[12], &TECHNIQUES),
        new_skill: named(record[13], &SKILLS),
        skill_uses: record[14..22].try_into().expect("8 bytes"),
    }
}

/// The whole file.
pub fn build(rom: &[u8]) -> Result<Json, PackError> {
    let mut tables = Vec::new();
    let mut research = Vec::new();
    let mut pointer_records = Vec::new();
    for (id, name) in CHARACTERS.iter().enumerate() {
        let at = POINTERS + id * POINTER_BYTES;
        let starting_level = word(rom, at)?;
        let table = long(rom, at + 2)? as usize;
        let count = LAST_LEVEL
            .checked_sub(starting_level)
            .ok_or_else(|| PackError(format!("{name} starts past level {LAST_LEVEL}")))?;
        pointer_records.push(
            Json::object()
                .with("character_id", id)
                .with("character", *name)
                .with("rom_offset", hex6(at))
                .with("starting_level", starting_level)
                .with("table_offset", hex6(table))
                .with("record_count", count)
                .with("raw_hex", hex(&rom[at..at + POINTER_BYTES])),
        );
        let levels = (0..usize::from(count))
            .map(|index| {
                let offset = table + index * RECORD_BYTES;
                let record = rom.get(offset..offset + RECORD_BYTES).ok_or_else(|| {
                    PackError(format!("Level record for {name} runs past end of ROM"))
                })?;
                Ok(level(record, starting_level + index as u16 + 1))
            })
            .collect::<Result<Vec<_>, PackError>>()?;
        tables.push(LevelTable {
            character_id: id as u8,
            character: (*name).to_owned(),
            starting_level,
            levels,
        });
        research.push(
            Json::object()
                .with("table_offset", hex6(table))
                .with("record_count", count),
        );
    }
    let total: usize = tables.iter().map(|t| t.levels.len()).sum();
    let starting: BTreeSet<u16> = tables.iter().map(|t| t.starting_level).collect();
    let census = Json::object()
        .with("starting_levels", starting.into_iter().collect::<Vec<_>>())
        .with(
            "levels_per_character",
            range(tables.iter().map(|t| t.levels.len() as u64)),
        )
        .with(
            "highest_level",
            tables
                .iter()
                .flat_map(|t| t.levels.iter().map(|l| l.level))
                .max(),
        )
        .with(
            "characters_learning_techniques",
            tables
                .iter()
                .filter(|t| t.levels.iter().any(|l| l.new_technique.is_some()))
                .count(),
        )
        .with(
            "characters_learning_skills",
            tables
                .iter()
                .filter(|t| t.levels.iter().any(|l| l.new_skill.is_some()))
                .count(),
        );
    let file = LevelsFile {
        format_version: FORMAT_VERSION,
        character_count: tables.len() as u32,
        total_records: total as u32,
        characters: tables,
    };
    let mut out = serialize(&file, "the levels")?;
    let extra = Json::object()
        .with("kind", "battle_levels")
        .with(
            "pointer_table",
            Json::object()
                .with("rom_offset", hex6(POINTERS))
                .with("record_size", POINTER_BYTES)
                .with("count", CHARACTERS.len())
                .with("records", pointer_records),
        )
        .with("characters", research)
        .with("census", census);
    overlay(&mut out, &extra);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_reads_its_words_bytes_and_names() {
        let mut raw = [0u8; RECORD_BYTES];
        raw[0..4].copy_from_slice(&0x0001_0203u32.to_be_bytes());
        raw[4..6].copy_from_slice(&300u16.to_be_bytes());
        raw[8] = 9;
        raw[12] = 2;
        raw[13] = 54;
        raw[21] = 7;
        let record = level(&raw, 5);
        assert_eq!(record.experience_required, 0x0001_0203);
        assert_eq!(record.hp, 300);
        assert_eq!(record.stats.strength, 9);
        assert_eq!(record.new_technique.unwrap().name.as_deref(), Some("Gifoi"));
        assert_eq!(record.new_skill.unwrap().name.as_deref(), Some("Recover2"));
        assert_eq!(record.skill_uses[7], 7);
    }

    #[test]
    fn zero_is_no_name_and_an_id_past_the_list_has_none() {
        assert!(named(0, &TECHNIQUES).is_none());
        let past = named(41, &TECHNIQUES).unwrap();
        assert_eq!((past.id, past.name), (41, None));
    }
}
