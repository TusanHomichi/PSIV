//! ROM-derived town destinations, dungeon exits, place-entry selectors and the
//! teleport windows' strings.

use std::{collections::BTreeSet, path::Path};

use serde::{Deserialize, Serialize};

use crate::{DataError, RomHash};

/// Location of the travel extract inside a runtime pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelFile {
    /// Relative JSON path.
    pub path: String,
    /// Extractor's provenance digest.
    pub sha256: String,
}

/// A destination shown by RYUKA's visited-town list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TownDestination {
    /// World byte: Motavia 0, Dezolis 1, Rykros 2.
    pub world: u8,
    /// Index in that world's teleport table.
    pub index: u8,
    /// Bit in the town flag bank.
    pub town_flag: u16,
    /// Decoded place name.
    pub name: String,
    /// Field_Map_Index_2 supplied at the overworld destination.
    pub previous_map: u16,
    /// Map_Start X in eight-pixel units.
    pub x: u16,
    /// Map_Start Y in eight-pixel units.
    pub y: u16,
}

/// An eight-byte DungeonTeleportPlaces record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DungeonDestination {
    /// Exit selector. Zero means unavailable.
    pub index: u8,
    /// Destination field map.
    pub map: u16,
    /// Map_Start X in eight-pixel units.
    pub x: u16,
    /// Map_Start Y in eight-pixel units.
    pub y: u16,
    /// Retail facing byte (0, 4, 8, 12).
    pub facing: u8,
    /// Retail party alignment byte.
    pub alignment: u8,
}

/// FieldRoutine_PlaceName matches both map words before applying a selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceEntry {
    /// Entered map.
    pub map: u16,
    /// Map from which the player entered.
    pub previous_map: u16,
    /// Decoded name displayed at entry.
    pub name: String,
    /// Town bit, $FE (clear exit), or $80 | dungeon exit.
    pub selector: u8,
}

/// The teleport windows' strings, decoded from the window charset with `$FC`
/// as a newline (`psiv_tools/travel.py` `MESSAGES` cites each one).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelMessages {
    /// The town list's label on each page's bottom border (`loc_2AA892`).
    pub towns_label: String,
    /// The line a granted teleport shows (`loc_2AA896`).
    pub teleported: String,
    /// The refusal where the place forbids a teleport (`loc_2AA8BC`).
    pub cannot_be_used: String,
    /// The refusal when the caster lacks the TP (`loc_2AA8D2`).
    pub not_enough_tp: String,
}

/// Validated travel tables. Old/synthetic packs may omit the entire file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelData {
    /// The file's version.
    pub format_version: u32,
    /// `field_travel`.
    pub kind: String,
    /// The ROM this came from.
    pub rom_sha256: RomHash,
    /// RYUKA rows in retail display order.
    pub towns: Vec<TownDestination>,
    /// HINAS rows keyed by their explicit index.
    pub dungeons: Vec<DungeonDestination>,
    /// Map-entry registration and inherited-exit selectors.
    pub entries: Vec<PlaceEntry>,
    /// The teleport windows' strings.
    pub messages: TravelMessages,
}

impl TravelData {
    pub(crate) fn load(
        directory: &Path,
        file: &TravelFile,
        rom: &RomHash,
    ) -> Result<Self, DataError> {
        let path = directory.join(&file.path);
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let data: Self = serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        data.validate(rom)
            .map_err(|message| DataError::ManifestMismatch {
                path,
                field: "travel",
                manifest:
                    "field_travel/2 with matching ROM, valid unique selectors and every message"
                        .into(),
                record: message,
            })?;
        Ok(data)
    }

    fn validate(&self, rom: &RomHash) -> Result<(), String> {
        if self.format_version != 2 || self.kind != "field_travel" || self.rom_sha256 != *rom {
            return Err("wrong version, kind or ROM hash".into());
        }
        let messages = &self.messages;
        for (key, text) in [
            ("towns_label", &messages.towns_label),
            ("teleported", &messages.teleported),
            ("cannot_be_used", &messages.cannot_be_used),
            ("not_enough_tp", &messages.not_enough_tp),
        ] {
            if text.is_empty() {
                return Err(format!("empty travel message {key}"));
            }
        }
        let mut towns = BTreeSet::new();
        for town in &self.towns {
            let (base, count) = match town.world {
                0 => (0, 16),
                1 => (16, 9),
                2 => (25, 1),
                _ => return Err("town world outside 0..3".into()),
            };
            if town.index >= count
                || town.town_flag != base + u16::from(town.index)
                || !towns.insert((town.world, town.index))
                || town.name.is_empty()
            {
                return Err(format!("invalid town {}:{}", town.world, town.index));
            }
        }
        let mut exits = BTreeSet::new();
        for exit in &self.dungeons {
            if exit.index >= 0x7F
                || !exits.insert(exit.index)
                || !matches!(exit.facing, 0 | 4 | 8 | 12)
                || exit.alignment > 3
            {
                return Err(format!("invalid dungeon exit {}", exit.index));
            }
        }
        let mut entries = BTreeSet::new();
        for entry in &self.entries {
            if !entries.insert((entry.map, entry.previous_map))
                || (entry.selector >= 0x80
                    && entry.selector != 0xFE
                    && !exits.contains(&(entry.selector & 0x7F)))
            {
                return Err(format!(
                    "invalid entry {}:{}",
                    entry.map, entry.previous_map
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn travel_rejects_duplicate_and_dangling_selectors_and_wrong_rom() {
        let rom = RomHash::parse(&"1".repeat(64)).unwrap();
        let mut data: TravelData = serde_json::from_value(serde_json::json!({
            "format_version":2, "kind":"field_travel", "rom_sha256":rom,
            "towns":[{"world":1,"index":0,"town_flag":16,"name":"RAJA TEMPLE","previous_map":332,"x":72,"y":190}],
            "dungeons":[{"index":1,"map":0,"x":92,"y":286,"facing":0,"alignment":0}],
            "entries":[{"map":16,"previous_map":0,"name":"PIATA","selector":129}],
            "messages":{"towns_label":"A","teleported":"B\nC","cannot_be_used":"D","not_enough_tp":"E"}
        })).unwrap();
        assert!(data.validate(&rom).is_ok());
        assert!(
            data.validate(&RomHash::parse(&"2".repeat(64)).unwrap())
                .is_err()
        );
        data.entries[0].selector = 130;
        assert!(data.validate(&rom).is_err());
        data.entries[0].selector = 129;
        data.towns.push(data.towns[0].clone());
        assert!(data.validate(&rom).is_err());
    }
}

#[cfg(test)]
mod message_tests {
    use super::*;

    fn data(rom: &RomHash) -> TravelData {
        serde_json::from_value(serde_json::json!({
            "format_version":2, "kind":"field_travel", "rom_sha256":rom,
            "towns":[], "dungeons":[], "entries":[],
            "messages":{"towns_label":"A","teleported":"B\nC","cannot_be_used":"D","not_enough_tp":"E"}
        }))
        .unwrap()
    }

    #[test]
    fn every_travel_message_must_be_present() {
        let rom = RomHash::parse(&"1".repeat(64)).unwrap();
        assert!(data(&rom).validate(&rom).is_ok());
        for field in 0..4 {
            let mut data = data(&rom);
            let messages = &mut data.messages;
            [
                &mut messages.towns_label,
                &mut messages.teleported,
                &mut messages.cannot_be_used,
                &mut messages.not_enough_tp,
            ][field]
                .clear();
            assert!(data.validate(&rom).is_err(), "field {field}");
        }
        // A pack from before the messages (format 1) is refused, not read
        // without them.
        let mut old = data(&rom);
        old.format_version = 1;
        assert!(old.validate(&rom).is_err());
        let missing = serde_json::from_value::<TravelData>(serde_json::json!({
            "format_version":2, "kind":"field_travel", "rom_sha256":rom,
            "towns":[], "dungeons":[], "entries":[]
        }));
        assert!(missing.is_err());
    }
}
