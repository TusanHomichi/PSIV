//! `travel.json`, HINAS and RYUKA destinations and `FieldRoutine_PlaceName`'s
//! entry records: the port of `psiv_tools/travel.py` (#216, slice 5).
//!
//! The file is `psiv_data::TravelData`. What the game doesn't read (each row's
//! `rom_offset`, a place id) has no typed home yet and is laid over it.

use std::path::Path;

use psiv_data::{DungeonDestination, PlaceEntry, RomHash, TownDestination, TravelData};
use sha2::{Digest, Sha256};

use crate::json::{Json, overlay};
use crate::png::hex;
use crate::text::{decode_name, split_terminated};
use crate::{PackError, Rom};

/// World, table, count. The Motavia loop reads 16 entries although only 13
/// precede the Dezolis table: its three aliases stay, rather than invented towns.
const TOWN_TABLES: [(u8, usize, usize); 3] = [(0, 0x6115A, 16), (1, 0x611C2, 9), (2, 0x6120A, 1)];
const DUNGEON_TABLE: usize = 0x61212;
const DUNGEON_COUNT: usize = 34;
const ENTRY_TABLE: usize = 0x664BC;
const ENTRY_COUNT: usize = 62;
const NAME_BLOCK: std::ops::Range<usize> = 0x2AB8A2..0x2ABA70;
const GUARDS: [(usize, &str); 6] = [
    (0x6115A, "00000010005c011e"),
    (0x611C2, "1800014c004800be"),
    (0x6120A, "2d0000f100800082"),
    (0x61322, "08f80000ec9b6600"),
    (ENTRY_TABLE, "001000000000001d00000101"),
    (ENTRY_TABLE + ENTRY_COUNT * 6, "ffff"),
];

fn word(raw: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([raw[at], raw[at + 1]])
}

fn row(rom: &[u8], offset: usize, size: usize) -> Result<&[u8], PackError> {
    rom.get(offset..offset + size)
        .ok_or_else(|| PackError(format!("a travel row at {offset:#x} runs past the ROM")))
}

/// `extract_travel`, and the research fields beside the typed record.
pub fn extract_travel(rom: &[u8], stamp: &str) -> Result<(TravelData, Json), PackError> {
    for (offset, expected) in GUARDS {
        let found = rom.get(offset..offset + expected.len() / 2).map(hex);
        if found.as_deref() != Some(expected) {
            return Err(PackError(format!(
                "travel table guard failed at {offset:#x}"
            )));
        }
    }
    let block = rom
        .get(NAME_BLOCK)
        .ok_or_else(|| PackError("the place names run past the ROM".into()))?;
    let names = split_terminated(block, 0xFE)?
        .into_iter()
        .map(|(_, raw)| decode_name(raw, "window").map_err(PackError::from))
        .collect::<Result<Vec<String>, PackError>>()?;
    let name = |place: u8| -> Result<String, PackError> {
        names
            .get(usize::from(place))
            .cloned()
            .ok_or_else(|| PackError(format!("place {place} is past the {} names", names.len())))
    };

    let (mut towns, mut town_notes) = (Vec::new(), Vec::new());
    for (world, base, count) in TOWN_TABLES {
        for index in 0..count {
            let offset = base + index * 8;
            let raw = row(rom, offset, 8)?;
            let town_flag = match world {
                0 => index as u16,
                1 => 16 + index as u16,
                _ => 25,
            };
            towns.push(TownDestination {
                world,
                index: index as u8,
                town_flag,
                name: name(raw[0])?,
                previous_map: word(raw, 2),
                x: word(raw, 4),
                y: word(raw, 6),
            });
            town_notes.push(
                Json::object()
                    .with("place_id", raw[0])
                    .with("rom_offset", offset),
            );
        }
    }
    let (mut dungeons, mut dungeon_notes) = (Vec::new(), Vec::new());
    for index in 0..DUNGEON_COUNT {
        let offset = DUNGEON_TABLE + index * 8;
        let raw = row(rom, offset, 8)?;
        dungeons.push(DungeonDestination {
            index: index as u8,
            map: word(raw, 0),
            x: word(raw, 2),
            y: word(raw, 4),
            facing: raw[6],
            alignment: raw[7],
        });
        dungeon_notes.push(Json::object().with("rom_offset", offset));
    }
    let (mut entries, mut entry_notes) = (Vec::new(), Vec::new());
    for index in 0..ENTRY_COUNT {
        let offset = ENTRY_TABLE + index * 6;
        let raw = row(rom, offset, 6)?;
        entries.push(PlaceEntry {
            map: word(raw, 0),
            previous_map: word(raw, 2),
            name: name(raw[4])?,
            selector: raw[5],
        });
        entry_notes.push(
            Json::object()
                .with("place_id", raw[4])
                .with("rom_offset", offset),
        );
    }
    let rom_sha256: RomHash = serde_json::from_value(stamp.into())
        .map_err(|error| PackError(format!("the ROM's hash doesn't read: {error}")))?;
    let data = TravelData {
        format_version: 1,
        kind: "field_travel".into(),
        rom_sha256,
        towns,
        dungeons,
        entries,
    };
    let research = Json::object()
        .with("towns", town_notes)
        .with("dungeons", dungeon_notes)
        .with("entries", entry_notes);
    Ok((data, research))
}

/// The section.
pub fn build(rom: &Rom, root: &Path) -> Result<(), PackError> {
    let stamp = hex(&Sha256::digest(rom.bytes()));
    let (data, research) = extract_travel(rom.bytes(), &stamp)?;
    let value = serde_json::to_value(&data)
        .map_err(|error| PackError(format!("cannot serialize travel.json: {error}")))?;
    let mut file = Json::from(&value);
    overlay(&mut file, &research);
    crate::write_json(root, "travel.json", &file).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rom_without_the_tables_is_refused_at_the_first_guard() {
        let error = extract_travel(&[0; 16], "").unwrap_err();
        assert!(error.0.contains("guard failed at 0x6115a"), "{}", error.0);
    }
}
