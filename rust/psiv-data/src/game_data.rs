//! Loading and validating a runtime pack into a [`GameData`].
//!
//! [`GameData::load`] reads a pack directory and [`GameData::from_parts`]
//! assembles one already in memory; both run the same validation, whose rule each
//! submodule states for its own concern: `load` reads the files, `maps` checks one
//! record's own numbers, `overworld` its page hooks, `pack` the manifest against the
//! record set and every warp against the map it targets, and `sprites` the sheet
//! index files and the references map records make into them.

mod load;
mod maps;
mod overworld;
mod pack;
mod sprites;

use self::maps::validate_map;
use self::pack::{validate_cross_references, validate_manifest};
use crate::error::DataError;
use crate::ids::MapId;
use crate::manifest::{Manifest, PACK_FORMAT_VERSION};
use crate::map::MapRecord;
use std::collections::BTreeMap;

/// A loaded, validated runtime pack.
///
/// Construct it with [`GameData::load`]. Once you hold one, every invariant the
/// `validate_*` functions in this module's submodules check is true of it: grids
/// match their declared dimensions, positions are inside their maps, warps point
/// somewhere the manifest knows about. Later layers can read it without
/// re-checking.
#[derive(Debug, Clone, PartialEq)]
pub struct GameData {
    manifest: Manifest,
    maps: BTreeMap<MapId, MapRecord>,
    /// Sheet id -> sheet, merged from `sprites/party.json` and
    /// `sprites/npcs.json` and the optional vehicle index. Empty for a
    /// pre-sprite pack built from parts.
    sheets: BTreeMap<String, crate::sprites::Sheet>,
    /// Party sheet ids in `CharFieldArtPtrs` order (Chaz first).
    party_sheet_ids: Vec<String>,
    /// Vehicle selector -> sheet id, in `Vehicle_Index` order.
    vehicle_sheet_ids: Vec<String>,
    /// Map id -> vehicle selector sheet ids for the map's CRAM line 3.
    vehicle_map_sheet_ids: BTreeMap<u16, Vec<String>>,
    sound: crate::sound::SoundFiles,
    new_game: Option<crate::NewGame>,
    travel: Option<crate::TravelData>,
}

impl GameData {
    /// Assemble and validate from parts already in memory.
    ///
    /// [`GameData::load`] is this plus the file reading. Useful for tests and
    /// for anything that gets a pack from somewhere other than a directory.
    pub fn from_parts(manifest: Manifest, records: Vec<MapRecord>) -> Result<GameData, DataError> {
        check_version(manifest.format_version)?;

        let mut maps: BTreeMap<MapId, MapRecord> = BTreeMap::new();
        for record in records {
            check_version(record.format_version)?;
            validate_map(&record)?;
            if let Some(previous) = maps.insert(record.id, record) {
                return Err(DataError::DuplicateMapId {
                    id: previous.id,
                    detail: format!("two map records claim to be {}", previous.label()),
                });
            }
        }

        validate_manifest(&manifest, &maps)?;
        validate_cross_references(&manifest, &maps)?;

        Ok(GameData {
            manifest,
            maps,
            sheets: BTreeMap::new(),
            party_sheet_ids: Vec::new(),
            vehicle_sheet_ids: Vec::new(),
            vehicle_map_sheet_ids: BTreeMap::new(),
            sound: crate::sound::SoundFiles::default(),
            new_game: None,
            travel: None,
        })
    }

    /// The pack index: the ROM hash, the inventory and the skipped maps.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The title's ROM-derived initializer, before any opening scenes run.
    /// Absent for synthetic packs built with `from_parts`.
    pub fn new_game(&self) -> Option<&crate::NewGame> {
        self.new_game.as_ref()
    }

    /// Field travel tables, absent in older or synthetic packs.
    pub fn travel(&self) -> Option<&crate::TravelData> {
        self.travel.as_ref()
    }

    /// All sprite sheets by id (party and NPC merged; ids never collide).
    pub fn sheets(&self) -> &BTreeMap<String, crate::sprites::Sheet> {
        &self.sheets
    }

    /// The extracted music, SFX, envelope and DAC records, if this pack has
    /// the sound wave installed.
    pub fn sound(&self) -> &crate::sound::SoundFiles {
        &self.sound
    }

    /// A sheet by id.
    pub fn sheet(&self, id: &str) -> Option<&crate::sprites::Sheet> {
        self.sheets.get(id)
    }

    /// Party sheets in `CharFieldArtPtrs` order: Chaz is `party_sheet(0)`.
    pub fn party_sheet(&self, slot: usize) -> Option<&crate::sprites::Sheet> {
        self.party_sheet_ids
            .get(slot)
            .and_then(|id| self.sheets.get(id))
    }

    /// The field sheet for a persisted vehicle selector (`1..=3`).
    pub fn vehicle_sheet(&self, index: u16) -> Option<&crate::sprites::Sheet> {
        index
            .checked_sub(1)
            .and_then(|slot| self.vehicle_sheet_ids.get(slot as usize))
            .and_then(|id| self.sheets.get(id))
    }

    /// The vehicle sheet for a map's CRAM line 3, falling back to the base
    /// selector sheet when an older or filtered pack has no variant entry.
    pub fn vehicle_sheet_for_map(&self, map_id: u16, index: u16) -> Option<&crate::sprites::Sheet> {
        let slot = index.checked_sub(1)? as usize;
        self.vehicle_map_sheet_ids
            .get(&map_id)
            .and_then(|ids| ids.get(slot))
            .and_then(|id| self.sheets.get(id))
            .or_else(|| self.vehicle_sheet(index))
    }

    /// The map with this id, if it is packed.
    pub fn map(&self, id: MapId) -> Option<&MapRecord> {
        self.maps.get(&id)
    }

    /// Every packed map, in ascending id order.
    pub fn maps(&self) -> impl Iterator<Item = (MapId, &MapRecord)> {
        self.maps.iter().map(|(id, record)| (*id, record))
    }

    /// Every packed map id, ascending.
    pub fn map_ids(&self) -> impl Iterator<Item = MapId> + '_ {
        self.maps.keys().copied()
    }

    /// How many maps are packed.
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    /// Whether the pack contains no maps at all.
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }

    /// Is this id packed?
    pub fn contains(&self, id: MapId) -> bool {
        self.maps.contains_key(&id)
    }

    /// Why this id was skipped, when the manifest says so. `None` for a packed
    /// id, for an unpacked warp target, and for one nobody has heard of.
    pub fn skip_reason(&self, id: MapId) -> Option<&str> {
        self.manifest.skip_reason(id)
    }

    /// Does the pack account for this id at all, whether by packing it or by
    /// declaring it unpacked? A warp target that fails this is a defect.
    pub fn knows(&self, id: MapId) -> bool {
        self.contains(id) || self.manifest.declares_unpacked(id)
    }
}

fn check_version(found: u32) -> Result<(), DataError> {
    if found != PACK_FORMAT_VERSION {
        return Err(DataError::FormatVersion {
            found,
            expected: PACK_FORMAT_VERSION,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
