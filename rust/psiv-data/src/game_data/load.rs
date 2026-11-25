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
use crate::mods::PackOverlay;
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
        Self::load_overlay(&PackOverlay::base(pack_dir))
    }

    /// Reads and validates a runtime pack through a [`PackOverlay`], resolving
    /// patched JSON tables in memory and mod assets over the base pack without
    /// touching disk.
    pub fn load_overlay(overlay: &PackOverlay) -> Result<GameData, DataError> {
        let manifest: Manifest = overlay.read_json_data("manifest.json")?;

        // Checked before anything else is read: if the shape changed, the map
        // files are not ours to interpret, and a parse error would be a
        // misleading way to find that out.
        check_version(manifest.format_version)?;

        let mut records = Vec::with_capacity(manifest.maps.len());
        for entry in &manifest.maps {
            let path = overlay.logical_path(&entry.json);
            let record: MapRecord = overlay.read_json_data(&entry.json)?;

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
            data.new_game = Some(crate::NewGame::load_overlay(overlay)?);
        }
        if let Some(file) = &data.manifest.travel {
            data.travel = Some(crate::TravelData::load_overlay(
                overlay,
                file,
                &data.manifest.rom.sha256,
            )?);
        }
        // Required: an older pack has no job board, so it is rebuilt, not run.
        let Some(file) = &data.manifest.guild else {
            return Err(DataError::ManifestMismatch {
                path: overlay.logical_path("manifest.json"),
                field: "guild",
                manifest: "absent; rebuild the pack".into(),
                record: "guild/guild.json".into(),
            });
        };
        {
            data.guild = Some(crate::GuildBoard::load_overlay(
                overlay,
                file,
                &data.manifest.rom.sha256,
            )?);
        }
        // Required as the guild's is: an older pack has no fifth-member table.
        let Some(file) = &data.manifest.fifth else {
            return Err(DataError::ManifestMismatch {
                path: overlay.logical_path("manifest.json"),
                field: "fifth",
                manifest: "absent; rebuild the pack".into(),
                record: "fifth/fifth.json".into(),
            });
        };
        data.fifth = Some(crate::FifthTable::load_overlay(
            overlay,
            file,
            &data.manifest.rom.sha256,
        )?);
        // Required: the battle stage loads the Fusion frames with the rest of
        // its art, so an older pack without them is rebuilt before any battle.
        crate::FusionArt::load_overlay(overlay)?;
        data.ending_credits = Some(crate::EndingCreditsAssets::load_overlay(
            overlay,
            data.manifest.rom.sha256.as_str(),
        )?);
        // A pack from an extractor that cut the Motavia table short is refused
        // here, before any battle draws from it (#40).
        data.battle_backgrounds = crate::BattleBackgrounds::load_overlay(overlay)?;
        if let Some(file) = &data.manifest.ship_menu {
            data.ship_menu = Some(crate::ShipMenu::load_overlay(
                overlay,
                file,
                &data.manifest.rom.sha256,
            )?);
        }
        // A pack extracted from a ROM carries its shops; a broken file fails
        // here and names itself. A synthetic pack has none.
        if overlay.is_file(crate::SHOPS_FILE) {
            data.shops = Some(crate::ShopData::load_overlay(overlay)?);
        }
        data.sound = crate::sound::SoundFiles::load_overlay(overlay)?;
        // Scene palettes: a pack extracted from a ROM carries them, and a
        // broken file fails here and names itself. A synthetic pack has none.
        if overlay.is_file(crate::PRESENTATION_FILE) {
            data.scene_palettes = crate::ScenePalettes::load_overlay(overlay)?;
        }
        if overlay.is_file(crate::SOUND_TEST_FILE) {
            data.sound_test = Some(crate::SoundTestFile::load_overlay(overlay)?);
        }
        // The message box walks these files, so a pack that has them carries
        // the whole set and a broken one fails here, where the error names the
        // file. A synthetic pack built from parts has no `dialogue/` directory
        // at all; the runtime refuses to build from data like that rather than
        // silently opening no window.
        if overlay.is_file("dialogue/trees.json") {
            data.dialogue = Some(std::sync::Arc::new(crate::DialogueSet::load_overlay(
                overlay,
            )?));
        }

        // Sprite index files (pack format 1). Loaded after the maps so NPC
        // sprite references can be validated against real sheets.
        let mut party_ids = Vec::new();
        let mut vehicle_ids = Vec::new();
        for (name, is_party, is_vehicle, optional) in [
            ("sprites/party.json", true, false, false),
            ("sprites/npcs.json", false, false, false),
            ("sprites/vehicles.json", false, true, true),
        ] {
            let path = overlay.logical_path(name);
            let text = match overlay.read_text(name) {
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
