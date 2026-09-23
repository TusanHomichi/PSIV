//! Loading and validating a runtime pack into a [`GameData`].

use crate::collision::{Collision, Plane};
use crate::error::DataError;
use crate::ids::MapId;
use crate::manifest::{Manifest, PACK_FORMAT_VERSION};
use crate::map::{
    COLLISION_CELL_PIXELS, CellPos, ContentsType, DIALOGUE_TREE_COUNT, Dimensions, MapRecord,
    Treasure, Warp,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// A loaded, validated runtime pack.
///
/// Construct it with [`GameData::load`]. Once you hold one, every invariant the
/// `validate_*` functions in this module check is true of it: grids match their
/// declared dimensions, positions are inside their maps, warps point somewhere
/// the manifest knows about. Later layers can read it without re-checking.
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

fn validate_sheet(sheet: &crate::sprites::Sheet) -> Result<(), DataError> {
    let fail = |message: String| DataError::Sprite {
        who: sheet.id.clone(),
        message,
    };
    if sheet.frame_width == 0 || sheet.frame_height == 0 || sheet.frame_count == 0 {
        return Err(fail(format!(
            "degenerate geometry {}x{} x{} frames",
            sheet.frame_width, sheet.frame_height, sheet.frame_count
        )));
    }
    if sheet.sequences.is_empty() {
        return Err(fail("no sequences".into()));
    }
    for (name, sequence) in &sheet.sequences {
        if sequence.frames.is_empty() {
            return Err(fail(format!("sequence {name} has no frames")));
        }
        for frame in &sequence.frames {
            if frame.index >= sheet.frame_count {
                return Err(fail(format!(
                    "sequence {name} names frame {} of a {}-frame strip",
                    frame.index, sheet.frame_count
                )));
            }
            if frame.duration_ticks == 0 {
                return Err(fail(format!("sequence {name} holds a frame for 0 ticks")));
            }
        }
    }
    Ok(())
}

fn validate_sprite_refs(data: &GameData) -> Result<(), DataError> {
    for (id, record) in &data.maps {
        for palette in record
            .map_effects
            .iter()
            .flat_map(|effect| &effect.paths)
            .flat_map(|path| &path.deferred_effects)
        {
            let reject = |message| DataError::Sprite {
                who: format!("map {id} palette copy"),
                message,
            };
            if palette.resolved_as != "palette_write" {
                return Err(reject(format!(
                    "unknown resolved effect {}",
                    palette.resolved_as
                )));
            }
            for replacement in &palette.affects.npc_sheets {
                let sprite = record
                    .npcs
                    .get(replacement.npc_index)
                    .and_then(|npc| npc.sprite.as_ref())
                    .ok_or_else(|| {
                        reject(format!("missing sprite for NPC {}", replacement.npc_index))
                    })?;
                if sprite.sheet != replacement.from {
                    return Err(reject(format!(
                        "NPC {} palette source disagrees with its base sheet",
                        replacement.npc_index
                    )));
                }
                let sheet = data
                    .sheets
                    .get(&replacement.to)
                    .ok_or_else(|| reject(format!("missing palette sheet {}", replacement.to)))?;
                for sequence in [&sprite.idle_sequence, &sprite.walk_sequence] {
                    if sheet.sequence(sequence).is_none() {
                        return Err(reject(format!(
                            "palette sheet {} lacks {sequence}",
                            replacement.to
                        )));
                    }
                }
            }
        }
        for chest in &record.treasure_chests {
            // Older packs did not extract chest art. Present references must
            // provide both lids; silently drawing frame zero hides open state.
            if let Some(sprite) = &chest.sprite {
                let who = format!("map {id} chest {}", chest.index);
                let sheet = data
                    .sheets
                    .get(&sprite.sheet)
                    .ok_or_else(|| DataError::Sprite {
                        who: who.clone(),
                        message: format!("missing chest sheet {}", sprite.sheet),
                    })?;
                for sequence in ["idle_down", "idle_up"] {
                    if sheet.sequence(sequence).is_none() {
                        return Err(DataError::Sprite {
                            who,
                            message: format!("missing chest lid {sequence}"),
                        });
                    }
                }
            }
        }
        for npc in &record.npcs {
            let who = format!("map {id} npc {}", npc.index);
            match (&npc.sprite, &npc.sprite_reason) {
                (Some(sprite), None) => {
                    let Some(sheet) = data.sheets.get(&sprite.sheet) else {
                        return Err(DataError::Sprite {
                            who,
                            message: format!("references missing sheet {}", sprite.sheet),
                        });
                    };
                    for sequence in [&sprite.idle_sequence, &sprite.walk_sequence] {
                        if sheet.sequence(sequence).is_none() {
                            return Err(DataError::Sprite {
                                who,
                                message: format!(
                                    "sheet {} has no sequence {sequence}",
                                    sprite.sheet
                                ),
                            });
                        }
                    }
                }
                (None, Some(_)) => {}
                (Some(_), Some(_)) | (None, None) => {
                    return Err(DataError::Sprite {
                        who,
                        message: "exactly one of sprite / sprite_reason must be set".into(),
                    });
                }
            }
        }
    }
    Ok(())
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

// ---------------------------------------------------------------------------
// Validation
//
// Always on, no lenient mode. A runtime that tolerates a broken pack stops
// being a test oracle for the cartridge (docs/RUNTIME_DESIGN.md, "Goal").
// Every failure names the map and the field, down to the array index.
// ---------------------------------------------------------------------------

fn validate_map(record: &MapRecord) -> Result<(), DataError> {
    let id = record.id;
    let dims = &record.dimensions;

    validate_dimensions(id, dims)?;
    validate_collision(id, dims, &record.collision)?;
    validate_overworld_patches(record)?;

    // The trees are numbered from one, so zero is as wrong as forty-four.
    if record.dialogue_tree == 0 || record.dialogue_tree > DIALOGUE_TREE_COUNT {
        return Err(DataError::validation(
            id,
            "dialogue_tree",
            format!(
                "tree {} does not exist; the ROM holds {DIALOGUE_TREE_COUNT} Kosinski dialogue \
                 trees, numbered 1..={DIALOGUE_TREE_COUNT}",
                record.dialogue_tree
            ),
        ));
    }

    for (index, npc) in record.npcs.iter().enumerate() {
        check_in_bounds(id, dims, npc.pos(), &format!("npcs[{index}]"))?;
    }

    for (index, chest) in record.treasure_chests.iter().enumerate() {
        check_in_bounds(id, dims, chest.pos(), &format!("treasure_chests[{index}]"))?;
        validate_treasure_contents(id, index, chest)?;
    }

    for (index, warp) in record.warps.iter().enumerate() {
        validate_warp_geometry(id, dims, index, warp)?;
    }

    for (index, area) in record.interaction_areas.iter().enumerate() {
        validate_interaction_geometry(id, dims, index, area)?;
    }

    Ok(())
}

fn validate_overworld_patches(record: &MapRecord) -> Result<(), DataError> {
    let id = record.id;
    let fail = |field: String, why: String| DataError::validation(id, field, why);
    if record.layout_patches.is_empty() {
        if matches!(id.0, 0 | 1) {
            return Err(fail(
                "layout_patches".into(),
                "retail overworld page hooks are missing; rebuild the pack".into(),
            ));
        }
        if record.overworld_patches.is_some() {
            return Err(fail(
                "overworld_patches".into(),
                "resolved page hooks exist without raw layout_patches".into(),
            ));
        }
        return Ok(());
    }
    let resolved = record.overworld_patches.as_ref().ok_or_else(|| {
        fail(
            "overworld_patches".into(),
            "raw page hooks need resolved tiles; rebuild this older runtime pack".into(),
        )
    })?;
    let atlas = record.patch_tiles.as_ref().ok_or_else(|| {
        fail(
            "patch_tiles".into(),
            "overworld page hooks need a composed atlas; rebuild the pack".into(),
        )
    })?;
    if record.png_over.is_none() || atlas.png_over.is_none() {
        return Err(fail(
            "patch_tiles.png_over".into(),
            "overworld patch must replace the priority overlay too".into(),
        ));
    }
    if atlas.tile_pixels != 32 || atlas.count as usize != atlas.tiles.len() {
        return Err(fail(
            "patch_tiles".into(),
            "invalid 32px atlas geometry".into(),
        ));
    }

    // The full-map atlas can precompose a flag's final FG/BG pair only when
    // distinct flags do not edit the same chunk. Repeated writes by one flag
    // retain the raw page-hook order; their final value is validated below.
    let mut owner = BTreeMap::<(u32, u32), u16>::new();
    let mut writes = BTreeMap::<(u16, u32, u32), (Option<u16>, Option<u16>)>::new();
    for (index, patch) in record.layout_patches.iter().enumerate() {
        let field = format!("layout_patches[{index}]");
        let flag = patch.event_flag.id;
        if flag >= 512 {
            return Err(fail(
                format!("{field}.event_flag.id"),
                format!("{flag} is outside event flags"),
            ));
        }
        let hex = patch
            .event_flag
            .id_hex
            .strip_prefix("0x")
            .and_then(|s| u16::from_str_radix(s, 16).ok());
        if hex != Some(flag) {
            return Err(fail(
                format!("{field}.event_flag.id_hex"),
                "hex flag id disagrees with numeric id".into(),
            ));
        }
        if patch.triggered_by_plane != "fg" && patch.triggered_by_plane != "bg" || patch.page >= 16
        {
            return Err(fail(
                field.clone(),
                "invalid page hook plane or page".into(),
            ));
        }
        if patch.writes.is_empty() {
            return Err(fail(format!("{field}.writes"), "empty page hook".into()));
        }
        for (write_index, write) in patch.writes.iter().enumerate() {
            let place = format!("{field}.writes[{write_index}]");
            if write.plane != "fg" && write.plane != "bg" {
                return Err(fail(format!("{place}.plane"), "expected fg or bg".into()));
            }
            if write.cell_x != write.chunk_x.saturating_mul(2)
                || write.cell_y != write.chunk_y.saturating_mul(2)
                || write.chunk_ids.is_empty()
            {
                return Err(fail(
                    place.clone(),
                    "cell/chunk geometry or chunk list is invalid".into(),
                ));
            }
            for (step, raw) in write.chunk_ids.iter().enumerate() {
                let x = write
                    .chunk_x
                    .checked_add(step as u32)
                    .ok_or_else(|| fail(place.clone(), "chunk coordinate overflow".into()))?;
                let y = write.chunk_y;
                if x >= record.dimensions.width_chunks || y >= record.dimensions.height_chunks {
                    return Err(fail(
                        place.clone(),
                        format!("chunk ({x},{y}) is outside the map"),
                    ));
                }
                let chunk = raw
                    .strip_prefix("0x")
                    .and_then(|s| u8::from_str_radix(s, 16).ok())
                    .ok_or_else(|| {
                        fail(
                            format!("{place}.chunk_ids[{step}]"),
                            format!("{raw:?} is not a raw chunk byte"),
                        )
                    })?;
                if let Some(previous) = owner.insert((x, y), flag)
                    && previous != flag
                {
                    return Err(fail(
                        place.clone(),
                        format!("chunk ({x},{y}) has overlapping event flags {previous}/{flag}"),
                    ));
                }
                let pair = writes.entry((flag, x, y)).or_default();
                if write.plane == "fg" {
                    pair.0 = Some(u16::from(chunk));
                } else {
                    pair.1 = Some(u16::from(chunk));
                }
            }
        }
    }

    let mut seen_flags = BTreeSet::new();
    let mut seen_tiles = BTreeSet::new();
    for (index, patch) in resolved.iter().enumerate() {
        let field = format!("overworld_patches[{index}]");
        if !seen_flags.insert(patch.event_flag) || patch.tiles.is_empty() {
            return Err(fail(
                field.clone(),
                "duplicate flag or empty tile group".into(),
            ));
        }
        for (tile_index, tile) in patch.tiles.iter().enumerate() {
            let place = format!("{field}.tiles[{tile_index}]");
            let key = (patch.event_flag, tile.chunk_x, tile.chunk_y);
            let Some((written_fg, written_bg)) = writes.get(&key) else {
                return Err(fail(place, "no matching raw page-hook write".into()));
            };
            if !seen_tiles.insert(key)
                || written_fg.is_some_and(|chunk| chunk != tile.fg_chunk_id)
                || written_bg.is_some_and(|chunk| chunk != tile.bg_chunk_id)
            {
                return Err(fail(
                    place.clone(),
                    "duplicate tile or final plane disagrees with raw writes".into(),
                ));
            }
            let authoritative = match record.collision.plane {
                Plane::Fg => tile.fg_chunk_id,
                Plane::Bg => tile.bg_chunk_id,
            };
            if tile.collision_chunk_id != authoritative || tile.collision.iter().any(|v| *v > 15) {
                return Err(fail(
                    place.clone(),
                    "collision plane/chunk/cells disagree".into(),
                ));
            }
            let unchanged_authority = match record.collision.plane {
                Plane::Fg => written_fg.is_none(),
                Plane::Bg => written_bg.is_none(),
            };
            if unchanged_authority {
                let base = record
                    .vehicle_battle
                    .as_ref()
                    .and_then(|grid| grid.rows.get(tile.chunk_y as usize))
                    .and_then(|row| row.get(tile.chunk_x as usize));
                if base != Some(&authoritative) {
                    return Err(fail(
                        place.clone(),
                        "unchanged collision plane disagrees with base chunk grid".into(),
                    ));
                }
            }
            let atlas_tile = atlas.tiles.get(tile.patch_tile as usize).ok_or_else(|| {
                fail(
                    format!("{place}.patch_tile"),
                    "atlas index is outside its tiles".into(),
                )
            })?;
            if atlas_tile.index != tile.patch_tile
                || atlas_tile.x != tile.patch_tile.saturating_mul(32)
                || atlas_tile.chunk_id.is_some()
                || atlas_tile.fg_chunk_id != Some(tile.fg_chunk_id)
                || atlas_tile.bg_chunk_id != Some(tile.bg_chunk_id)
                || atlas_tile.collision != Some(tile.collision)
            {
                return Err(fail(
                    place,
                    "composed atlas tile disagrees with patch".into(),
                ));
            }
        }
    }
    if seen_tiles.len() != writes.len() {
        return Err(fail(
            "overworld_patches".into(),
            "resolved tile set does not cover every raw page-hook coordinate".into(),
        ));
    }
    Ok(())
}

fn validate_dimensions(id: MapId, dims: &Dimensions) -> Result<(), DataError> {
    if dims.width_cells == 0 || dims.height_cells == 0 {
        return Err(DataError::validation(
            id,
            "dimensions",
            format!(
                "a map cannot be {}x{} cells",
                dims.width_cells, dims.height_cells
            ),
        ));
    }
    if dims.cell_pixels != COLLISION_CELL_PIXELS {
        return Err(DataError::validation(
            id,
            "dimensions.cell_pixels",
            format!(
                "collision cells are {COLLISION_CELL_PIXELS} pixels, not {}; the whole fidelity \
                 spine is built on the 16-pixel cell grid",
                dims.cell_pixels
            ),
        ));
    }
    // A chunk is 4x4 tiles and a cell is 2x2, so the three ways the pack states
    // a map's size are one fact. Any disagreement means the packer's layout and
    // its collision decode came apart.
    for (axis, cells, chunks, pixels) in [
        (
            "width",
            dims.width_cells,
            dims.width_chunks,
            dims.width_pixels,
        ),
        (
            "height",
            dims.height_cells,
            dims.height_chunks,
            dims.height_pixels,
        ),
    ] {
        let from_chunks = chunks.saturating_mul(2);
        if cells != from_chunks {
            return Err(DataError::validation(
                id,
                format!("dimensions.{axis}_cells"),
                format!("{chunks} chunks is {from_chunks} cells, but the pack says {cells}"),
            ));
        }
        let expected = cells.saturating_mul(dims.cell_pixels);
        if pixels != expected {
            return Err(DataError::validation(
                id,
                format!("dimensions.{axis}_pixels"),
                format!(
                    "{cells} cells at {} pixels is {expected} pixels, but the pack says {pixels}",
                    dims.cell_pixels
                ),
            ));
        }
    }
    Ok(())
}

fn validate_collision(id: MapId, dims: &Dimensions, section: &Collision) -> Result<(), DataError> {
    let grid = &section.grid;
    if grid.width() != dims.width_cells || grid.height() != dims.height_cells {
        return Err(DataError::validation(
            id,
            "collision",
            format!(
                "the grid is {}x{} cells but the map declares {}x{}",
                grid.width(),
                grid.height(),
                dims.width_cells,
                dims.height_cells
            ),
        ));
    }
    Ok(())
}

fn validate_treasure_contents(id: MapId, index: usize, chest: &Treasure) -> Result<(), DataError> {
    // Byte 1 of the chest record selects how byte 3 is read. If the pack says
    // "item" and carries a meseta amount, one of the two is a lie and we cannot
    // tell which.
    let (present, absent, expected) = match chest.contents_type {
        ContentsType::Item => (chest.item_id.is_some(), chest.meseta.is_some(), "item_id"),
        ContentsType::Meseta => (chest.meseta.is_some(), chest.item_id.is_some(), "meseta"),
    };
    if !present {
        return Err(DataError::validation(
            id,
            format!("treasure_chests[{index}].{expected}"),
            format!(
                "contents_type is {:?} but {expected} is null",
                chest.contents_type
            ),
        ));
    }
    if absent {
        return Err(DataError::validation(
            id,
            format!("treasure_chests[{index}]"),
            format!(
                "contents_type is {:?} but both item_id and meseta are set",
                chest.contents_type
            ),
        ));
    }
    Ok(())
}

fn validate_warp_geometry(
    id: MapId,
    dims: &Dimensions,
    index: usize,
    warp: &Warp,
) -> Result<(), DataError> {
    // `rect` is the trigger area and is the thing that has to be on the map;
    // the packer clips it. `source` is the raw record coordinate, which the
    // standing-cell shift can push one row past the bottom edge, so it is not
    // bounds-checked -- see the module note on `WarpSource`.
    let Some(rect) = warp.rect else {
        return Ok(());
    };

    if rect.width == 0 || rect.height == 0 {
        return Err(DataError::validation(
            id,
            format!("warps[{index}].rect"),
            format!(
                "a {}x{} trigger rectangle can never fire; the packer emits no rectangle at all \
                 for an empty area",
                rect.width, rect.height
            ),
        ));
    }
    let end = rect.end();
    if end.x > dims.width_cells || end.y > dims.height_cells {
        return Err(DataError::validation(
            id,
            format!("warps[{index}].rect"),
            format!(
                "cells {}..{} x {}..{} run outside the {}x{} map; the packer must clip \
                 open-ended XYRange entries such as {} to the map",
                rect.x, end.x, rect.y, end.y, dims.width_cells, dims.height_cells, warp.range.name
            ),
        ));
    }
    Ok(())
}

fn validate_interaction_geometry(
    id: MapId,
    dims: &Dimensions,
    index: usize,
    area: &crate::map::InteractionArea,
) -> Result<(), DataError> {
    let Some(rect) = area.rect else {
        return Ok(());
    };
    if rect.width == 0 || rect.height == 0 {
        return Err(DataError::validation(
            id,
            format!("interaction_areas[{index}].rect"),
            "an interaction area cannot have zero width or height",
        ));
    }
    let end = rect.end();
    if end.x > dims.width_cells || end.y > dims.height_cells {
        return Err(DataError::validation(
            id,
            format!("interaction_areas[{index}].rect"),
            format!(
                "cells {}..{} x {}..{} run outside the {}x{} map",
                rect.x, end.x, rect.y, end.y, dims.width_cells, dims.height_cells
            ),
        ));
    }
    Ok(())
}

fn check_in_bounds(
    id: MapId,
    dims: &Dimensions,
    pos: CellPos,
    field: &str,
) -> Result<(), DataError> {
    if pos.x >= dims.width_cells || pos.y >= dims.height_cells {
        return Err(DataError::validation(
            id,
            field.to_string(),
            format!(
                "cell ({}, {}) is outside the {}x{} map",
                pos.x, pos.y, dims.width_cells, dims.height_cells
            ),
        ));
    }
    Ok(())
}

fn validate_manifest(
    manifest: &Manifest,
    maps: &BTreeMap<MapId, MapRecord>,
) -> Result<(), DataError> {
    let mut seen: BTreeMap<MapId, &'static str> = BTreeMap::new();
    for entry in &manifest.maps {
        if seen.insert(entry.id, "maps").is_some() {
            return Err(DataError::DuplicateMapId {
                id: entry.id,
                detail: "listed twice in `maps`".to_string(),
            });
        }
    }
    for entry in &manifest.skipped {
        if let Some(previous) = seen.insert(entry.id, "skipped") {
            return Err(DataError::DuplicateMapId {
                id: entry.id,
                detail: format!("listed in `skipped` and already in `{previous}`"),
            });
        }
    }

    // The manifest and the record set have to describe the same pack. Either
    // direction of drift means something was packed or unpacked without the
    // index being told.
    for entry in &manifest.maps {
        if !maps.contains_key(&entry.id) {
            return Err(DataError::validation(
                entry.id,
                "manifest.maps",
                format!(
                    "listed in the manifest as {} but no record was loaded",
                    entry.json
                ),
            ));
        }
    }
    for id in maps.keys() {
        if !manifest.packs(*id) {
            return Err(DataError::validation(
                *id,
                "manifest.maps",
                "a record was loaded for a map the manifest does not list",
            ));
        }
    }
    Ok(())
}

fn validate_cross_references(
    manifest: &Manifest,
    maps: &BTreeMap<MapId, MapRecord>,
) -> Result<(), DataError> {
    for (id, record) in maps {
        for (index, warp) in record.warps.iter().enumerate() {
            let target_id = warp.target.id;
            match maps.get(&target_id) {
                Some(target) => {
                    // The destination has to exist on the map the player lands
                    // on, not merely on this one.
                    let landing = warp.destination.pos();
                    if !target.contains(landing) {
                        return Err(DataError::validation(
                            *id,
                            format!("warps[{index}].destination"),
                            format!(
                                "cell ({}, {}) is outside map {} ({}), which is {}x{} cells",
                                landing.x,
                                landing.y,
                                target_id,
                                target.label(),
                                target.dimensions.width_cells,
                                target.dimensions.height_cells
                            ),
                        ));
                    }
                }
                None if manifest.declares_unpacked(target_id) => {}
                None => {
                    return Err(DataError::validation(
                        *id,
                        format!("warps[{index}].target"),
                        format!(
                            "targets map {target_id} ({}), which is neither packed nor listed in \
                             the manifest's `skipped` or `unpacked_warp_targets`",
                            warp.target.label()
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "game_data_tests.rs"]
mod tests;
