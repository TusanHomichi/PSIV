//! `Event_PickingFifthCharacter`'s tables, as the pack carries them
//! (`psiv_tools/fifth_pack.py`): the five friends waiting at the Mota Spaceport
//! and the records the routine refills.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{DataError, RomHash};

/// How many friends the table holds.
pub const FIFTH_ROWS: usize = 5;

/// Where the pack keeps the table's JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FifthFile {
    /// Relative JSON path.
    pub path: String,
    /// Extractor's provenance digest.
    pub sha256: String,
}

/// One row of `loc_72182`, without its art byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct FifthRowRecord {
    /// Where the friend stands: X in pixels.
    pub x: u16,
    /// Y in pixels.
    pub y: u16,
    /// The character id.
    pub character: u8,
    /// The first of the friend's dialogue lines.
    pub first_line: u8,
    /// The `EventFlag_*Picked` set while this friend is the fifth.
    pub picked_flag: u8,
}

#[derive(Deserialize)]
struct FifthJson {
    format_version: u32,
    kind: String,
    rom_sha256: RomHash,
    rows: Vec<FifthRowRecord>,
    records: Vec<u8>,
}

/// The routine's tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FifthTable {
    /// `loc_72182`, in the order of the friends' objects.
    pub rows: [FifthRowRecord; FIFTH_ROWS],
    /// `loc_721B8`: the character records refilled, as ids.
    pub records: [u8; FIFTH_ROWS],
}

impl FifthTable {
    pub(crate) fn load(
        directory: &Path,
        file: &FifthFile,
        rom: &RomHash,
    ) -> Result<FifthTable, DataError> {
        let path = directory.join(&file.path);
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let json: FifthJson = serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        let refused = || DataError::ManifestMismatch {
            path: path.clone(),
            field: "fifth",
            manifest: "fifth/1 with matching ROM, five rows and five records".into(),
            record: "wrong version, kind, ROM hash or a count".into(),
        };
        if json.format_version != 1 || json.kind != "fifth" || json.rom_sha256 != *rom {
            return Err(refused());
        }
        Ok(FifthTable {
            rows: json.rows.try_into().map_err(|_| refused())?,
            records: json.records.try_into().map_err(|_| refused())?,
        })
    }
}
