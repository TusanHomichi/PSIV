//! ROM-derived town destinations, dungeon exits and place-entry selectors.

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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

/// Validated travel tables. Old/synthetic packs may omit the entire file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TravelData {
    format_version: u32,
    kind: String,
    rom_sha256: RomHash,
    /// RYUKA rows in retail display order.
    pub towns: Vec<TownDestination>,
    /// HINAS rows keyed by their explicit index.
    pub dungeons: Vec<DungeonDestination>,
    /// Map-entry registration and inherited-exit selectors.
    pub entries: Vec<PlaceEntry>,
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
                manifest: "field_travel/1 with matching ROM and valid unique selectors".into(),
                record: message,
            })?;
        Ok(data)
    }

    fn validate(&self, rom: &RomHash) -> Result<(), String> {
        if self.format_version != 1 || self.kind != "field_travel" || self.rom_sha256 != *rom {
            return Err("wrong version, kind or ROM hash".into());
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
            "format_version":1, "kind":"field_travel", "rom_sha256":rom,
            "towns":[{"world":1,"index":0,"town_flag":16,"name":"RAJA TEMPLE","previous_map":332,"x":72,"y":190}],
            "dungeons":[{"index":1,"map":0,"x":92,"y":286,"facing":0,"alignment":0}],
            "entries":[{"map":16,"previous_map":0,"name":"PIATA","selector":129}]
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
