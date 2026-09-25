//! Reading a runtime pack directory.
//!
//! The manifest comes first and its format version is checked before any map is
//! read, so a pack of another shape is refused for what it is rather than for the
//! parse error a later file would raise. The sprite index files are read last, once
//! there are sheets for the NPC references the map records carry to be checked
//! against.

use super::sprites::{validate_sheet, validate_sprite_refs};
use super::{GameData, check_version};
use crate::error::DataError;
use crate::manifest::Manifest;
use crate::map::MapRecord;
use std::path::Path;

impl GameData {
    /// Read `manifest.json` from `pack_dir` and every map it lists, then
    /// validate the lot.
    ///
    /// Fails on a format-version mismatch, a missing or unreadable file,
    /// malformed JSON, or any validation failure. There is no partial success:
    /// either the whole pack is coherent, or you get an error naming what is
    /// not.
    pub fn load(pack_dir: &Path) -> Result<GameData, DataError> {
        let manifest_path = pack_dir.join("manifest.json");
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|e| DataError::io(&manifest_path, e))?;
        let manifest: Manifest =
            serde_json::from_str(&text).map_err(|e| DataError::json(&manifest_path, e))?;

        // Checked before anything else is read: if the shape changed, the map
        // files are not ours to interpret, and a parse error would be a
        // misleading way to find that out.
        check_version(manifest.format_version)?;

        let mut records = Vec::with_capacity(manifest.maps.len());
        for entry in &manifest.maps {
            let path = pack_dir.join(&entry.json);
            let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
            let record: MapRecord =
                serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;

            check_version(record.format_version)?;
            if record.id != entry.id {
                return Err(DataError::ManifestMismatch {
                    path,
                    field: "id",
                    manifest: entry.id.to_string(),
                    record: record.id.to_string(),
                });
            }
            if entry.symbol.is_some() && record.symbol.is_some() && entry.symbol != record.symbol {
                return Err(DataError::ManifestMismatch {
                    path,
                    field: "symbol",
                    manifest: format!("{:?}", entry.symbol.as_deref().unwrap_or("")),
                    record: format!("{:?}", record.symbol.as_deref().unwrap_or("")),
                });
            }
            records.push(record);
        }

        let mut data = GameData::from_parts(manifest, records)?;
        if data.manifest.game_start.is_some() {
            data.new_game = Some(crate::NewGame::load(pack_dir)?);
        }
        if let Some(file) = &data.manifest.travel {
            data.travel = Some(crate::TravelData::load(
                pack_dir,
                file,
                &data.manifest.rom.sha256,
            )?);
        }
        data.sound = crate::sound::SoundFiles::load(pack_dir)?;

        // Sprite index files (pack format 1). Loaded after the maps so NPC
        // sprite references can be validated against real sheets.
        let mut party_ids = Vec::new();
        let mut vehicle_ids = Vec::new();
        for (name, is_party, is_vehicle, optional) in [
            ("sprites/party.json", true, false, false),
            ("sprites/npcs.json", false, false, false),
            ("sprites/vehicles.json", false, true, true),
        ] {
            let path = pack_dir.join(name);
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) if optional && error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(DataError::io(&path, error)),
            };
            let file: crate::sprites::SheetFile =
                serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
            check_version(file.format_version)?;
            if file.sheet_count as usize != file.sheets.len() {
                return Err(DataError::ManifestMismatch {
                    path,
                    field: "sheet_count",
                    manifest: file.sheet_count.to_string(),
                    record: file.sheets.len().to_string(),
                });
            }
            for sheet in file.sheets {
                validate_sheet(&sheet)?;
                if is_party {
                    party_ids.push(sheet.id.clone());
                }
                if is_vehicle {
                    vehicle_ids.push(sheet.id.clone());
                }
                if let Some(previous) = data.sheets.insert(sheet.id.clone(), sheet) {
                    return Err(DataError::Sprite {
                        who: previous.id,
                        message: "sheet id appears in more than one index file".into(),
                    });
                }
            }
            if is_vehicle {
                for variant in file.palette_variants {
                    if variant.sheet_ids.len() != vehicle_ids.len() {
                        return Err(DataError::Sprite {
                            who: format!("vehicle palette maps {:?}", variant.map_ids),
                            message: format!(
                                "has {} sheet ids; expected {}",
                                variant.sheet_ids.len(),
                                vehicle_ids.len()
                            ),
                        });
                    }
                    for sheet in variant.sheets {
                        validate_sheet(&sheet)?;
                        if !variant.sheet_ids.iter().any(|id| id == &sheet.id) {
                            return Err(DataError::Sprite {
                                who: sheet.id,
                                message: "vehicle palette sheet is not named by sheet_ids".into(),
                            });
                        }
                        if let Some(previous) = data.sheets.insert(sheet.id.clone(), sheet) {
                            return Err(DataError::Sprite {
                                who: previous.id,
                                message: "sheet id appears in more than one index file".into(),
                            });
                        }
                    }
                    for map_id in variant.map_ids {
                        if data
                            .vehicle_map_sheet_ids
                            .insert(map_id, variant.sheet_ids.clone())
                            .is_some()
                        {
                            return Err(DataError::Sprite {
                                who: format!("map {map_id:#05x}"),
                                message: "vehicle palette is declared more than once".into(),
                            });
                        }
                    }
                }
            }
        }
        data.party_sheet_ids = party_ids;
        data.vehicle_sheet_ids = vehicle_ids;
        validate_sprite_refs(&data)?;
        Ok(data)
    }
}
