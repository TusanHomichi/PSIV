//! The `maps` section: `maps/`, `sprites/` and then the shops, built from the ROM. Sheets are
//! registered first, in map order, because the first object to ask names a
//! sheet's file. Then each map's pictures and file are built in parallel: one
//! map's work is [`map_pictures`] and [`super::file::map_file`], which together
//! are [retired pack](https://github.com/TusanHomichi/PSIV/blob/902868b517fd6b8eb0aee15b5dd500623cc7954c/psiv_tools/pack.py)'s map loop.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use sha2::{Digest, Sha256};

use super::art_cycle::{TileDmaFrames, extract_tile_dma_frames, render_art_cycle_frames};
use super::chunk_def_copy::resolve_chunk_def_copies;
use super::draw::{PriorityCounts, priority_overlay, render_layout};
use super::effects::{MapEffects, extract_map_effects, field, int, text};
use super::file::map_file;
use super::layouts::{
    Layout, MapLayout, Plane, decode_collision, map_palette_rgb, unloaded_patterns,
};
use super::manifest::{self, MapPart};
use super::overworld::{MapSection, decode_map_section};
use super::patches::{
    PatchCounts, atlas_json, index_overworld_patches, index_writes, patch_atlas,
    resolve_map_effects, resolve_overworld_patches, scene_patch_chunks, write_composite_pairs,
};
use super::records::MapRecord;
use super::updates::{MapUpdates, conveyor_belt_chunks, extract_map_updates};
use super::{ExtractedMaps, extract_maps};
use crate::decode::{Call, Decodes};
use crate::json::Json;
use crate::png::{self, hex};
use crate::shops::ShopMap;
use crate::sprites::emit::{MapSprites, field_sprites, write_sprites};
use crate::{PackError, Rom};

/// A map's file stem.
pub fn map_stem(record: &MapRecord) -> String {
    format!("{:03X}_{}", record.id, record.symbol)
}

fn rows(cells: &[u8], width: usize) -> Vec<Json> {
    cells
        .chunks_exact(width)
        .map(|row| Json::from(row.to_vec()))
        .collect()
}

#[allow(clippy::too_many_arguments)]
/// `layout_variants`: the alternate layouts a `layout_replace` swaps in, each a
/// first-class layout. Appends the pictures to `files`.
fn layout_variants(
    decodes: &Decodes,
    record: &MapRecord,
    decoded: &MapLayout,
    replacements: &[&Json],
    palette: &[[u8; 3]],
    stem: &str,
    dma: Option<&TileDmaFrames>,
    files: &mut Vec<(String, Vec<u8>)>,
    art_cycle_images: &mut BTreeMap<String, Vec<String>>,
) -> Result<Vec<Json>, PackError> {
    if replacements.is_empty() {
        return Ok(Vec::new());
    }
    let spec = &decoded.spec;
    let mut fg = decoded.fg.clone();
    let mut bg = decoded.bg.clone();
    let mut changed = Vec::with_capacity(replacements.len());
    for replacement in replacements {
        let name = text(replacement, "plane");
        let source_text = text(replacement, "source");
        let source = usize::from_str_radix(source_text.trim_start_matches("0x"), 16)
            .map_err(|_| PackError(format!("a replacement source reads {source_text}")))?;
        let (plane, base): (&mut Layout, usize) = if name == Plane::Fg.name() {
            (&mut fg, spec.layout_fg)
        } else {
            (&mut bg, spec.layout_bg)
        };
        let cells = decodes.get(Call::Kosinski { offset: source })?;
        let cells = cells.bytes();
        let expected = plane.width_chunks * plane.height_chunks;
        if cells.len() != expected {
            return Err(PackError(format!(
                "map 0x{:03X}: the {} replacement at 0x{source:06X} decompresses to {} bytes, not \
                 the {expected} a {}x{} grid needs",
                record.id,
                name.to_uppercase(),
                cells.len(),
                plane.width_chunks,
                plane.height_chunks
            )));
        }
        plane.cells = cells.to_vec();
        plane.anomaly = None;
        changed.push(
            Json::object()
                .with("plane", name)
                .with("source", source_text)
                .with("identical_to_base", source == base),
        );
    }
    let collision = decode_collision(
        &decoded.chunks,
        match spec.collision_plane() {
            Plane::Bg => &bg,
            Plane::Fg => &fg,
        },
    )?;
    let variant = MapLayout {
        spec: spec.clone(),
        chunks: decoded.chunks.clone(),
        fg,
        bg,
        collision,
        patterns: decoded.patterns.clone(),
    };
    let image = render_layout(
        &variant.chunks,
        &variant.bg,
        &variant.fg,
        &variant.patterns,
        palette,
    )?;
    let (overlay, counts) = priority_overlay(&variant, palette)?;
    let png_name = format!("maps/{stem}_variant.png");
    let over_name = overlay
        .is_some()
        .then(|| format!("maps/{stem}_variant_over.png"));
    let grid = &variant.collision;
    let layout = variant.collision_layout();
    let differs = decoded
        .collision
        .types
        .iter()
        .zip(&grid.types)
        .filter(|(a, b)| a != b)
        .count();
    let json = Json::object()
        .with("id", 0)
        .with("planes", changed)
        .with("png", png_name.as_str())
        .with("png_over", over_name.as_deref())
        .with("png_sha256", png::image_sha256(&image)?)
        .with(
            "png_over_sha256",
            overlay.as_deref().map(png::image_sha256).transpose()?,
        )
        .with("priority_tiles", counts.tiles_fg + counts.tiles_bg)
        .with("unloaded_patterns", unloaded_patterns(&variant)?)
        .with(
            "collision",
            Json::object()
                .with("plane", spec.collision_plane().name())
                .with("width_cells", grid.width)
                .with("height_cells", grid.height)
                .with("rows", rows(&grid.types, grid.width))
                .with("sha256", hex(&Sha256::digest(&grid.types))),
        )
        .with(
            "vehicle_battle",
            Json::object()
                .with("plane", layout.plane.name())
                .with("width_chunks", layout.width_chunks)
                .with("height_chunks", layout.height_chunks)
                .with("rows", rows(&layout.cells, layout.width_chunks)),
        )
        .with("differs_from_base_cells", differs);
    let base_ref = image.clone();
    let over_ref = overlay.clone();
    files.push((png_name.clone(), image));
    if let (Some(name), Some(image)) = (over_name.clone(), overlay) {
        files.push((name, image));
    }
    if let Some(dma) = dma {
        render_art_cycle_frames(
            &variant,
            palette,
            dma,
            &png_name,
            &base_ref,
            over_name.as_deref(),
            over_ref.as_deref(),
            files,
            art_cycle_images,
        )?;
    }
    Ok(vec![json])
}

/// One map's pictures and the JSON that goes with them.
pub struct Pictures {
    /// `(pack-relative path, PNG)`, every picture this map has.
    pub files: Vec<(String, Vec<u8>)>,
    pub map_effects: Vec<Json>,
    pub patch_counts: PatchCounts,
    pub overworld_patches: Vec<Json>,
    pub chunk_def_copies: Vec<Json>,
    pub scene_chunks: Vec<u8>,
    pub variants: Vec<Json>,
    pub patch_tiles: Option<Json>,
    /// `map_update_images`, when an update needs palette-index renders.
    pub update_images: Option<Vec<(String, String)>>,
    /// `art_cycle_images`, when a tile-DMA cycle swaps pattern blocks on this map.
    pub art_cycle_images: BTreeMap<String, Vec<String>>,
    /// The above-sprites layer's counts, and whether it has a file.
    pub priority: PriorityCounts,
    pub has_overlay: bool,
    /// The chunks a write or scene patch changes; whether a patch atlas was
    /// built, and whether it has an above-sprites half.
    pub patched_chunks: usize,
    pub atlas: bool,
    pub atlas_over: bool,
}

/// `bind_updates`: only programs that rewrite CRAM need palette-index renders.
fn needs_index_images(programs: &[Json]) -> bool {
    programs.iter().any(|entry| {
        field(entry, "program").is_some_and(|program| {
            matches!(
                text(program, "kind"),
                "palette" | "crystals" | "edge" | "edge_line"
            )
        })
    })
}

/// One map's pictures, as `build_pack`'s loop makes them. The index images come
/// last: they are every other picture with its palette rewritten. `entries` is
/// the map's effect list, with its palette effects on it.
#[allow(clippy::too_many_arguments)]
pub fn map_pictures(
    rom: &[u8],
    decodes: &Decodes,
    record: &MapRecord,
    section: &MapSection,
    effects: &MapEffects,
    entries: &[Json],
    updates: &MapUpdates,
) -> Result<Pictures, PackError> {
    let decoded = section.layout();
    let mut palette = map_palette_rgb(rom, decoded.spec.palette)?;
    palette.truncate(32);
    let stem = map_stem(record);
    let programs = updates
        .per_map
        .get(&record.id)
        .map_or(&[][..], Vec::as_slice);
    let dma = extract_tile_dma_frames(rom, decodes, record, programs, &decoded.patterns)?;
    let image = render_layout(
        &decoded.chunks,
        &decoded.bg,
        &decoded.fg,
        &decoded.patterns,
        &palette,
    )?;
    let (overlay, priority) = priority_overlay(decoded, &palette)?;
    let base_png_name = format!("maps/{stem}.png");
    let over_png_name = format!("maps/{stem}_over.png");
    let base_ref = dma.as_ref().map(|_| image.clone());
    let over_ref = dma.as_ref().and_then(|_| overlay.clone());
    let mut files = vec![(base_png_name.clone(), image)];
    let has_overlay = overlay.is_some();
    if let Some(overlay) = overlay {
        files.push((over_png_name.clone(), overlay));
    }
    let mut art_cycle_images = BTreeMap::new();
    if let (Some(dma), Some(base_bytes)) = (dma.as_ref(), base_ref.as_deref()) {
        render_art_cycle_frames(
            decoded,
            &palette,
            dma,
            &base_png_name,
            base_bytes,
            has_overlay.then_some(over_png_name.as_str()),
            over_ref.as_deref(),
            &mut files,
            &mut art_cycle_images,
        )?;
    }

    let replacements: Vec<&Json> = effects
        .replacements
        .iter()
        .filter(|r| int(r, "map") == Some(record.id as i128))
        .collect();
    let variants = layout_variants(
        decodes,
        record,
        decoded,
        &replacements,
        &palette,
        &stem,
        dma.as_ref(),
        &mut files,
        &mut art_cycle_images,
    )?;

    let (mut map_effects, written, patch_counts) = resolve_map_effects(decoded, entries)?;
    let (mut overworld_patches, composite_pairs) =
        resolve_overworld_patches(decoded, section.overworld())?;
    let (mut chunk_def_copies, chunk_copy_pairs) = resolve_chunk_def_copies(rom, record, decoded)?;
    if (!composite_pairs.is_empty() || !chunk_copy_pairs.is_empty()) && !has_overlay {
        return Err(PackError(format!(
            "map 0x{:03X}: overworld hook needs a base priority overlay",
            record.id
        )));
    }
    let composite_pairs: Vec<(u8, u8)> = composite_pairs
        .into_iter()
        .chain(chunk_copy_pairs)
        .chain(write_composite_pairs(&map_effects))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let scene_chunks = scene_patch_chunks(rom, record)?;
    let belt_chunks = updates
        .per_map
        .get(&record.id)
        .map(|entries| conveyor_belt_chunks(entries))
        .unwrap_or_default();
    let patched: BTreeSet<u8> = written
        .iter()
        .chain(&scene_chunks)
        .chain(&belt_chunks)
        .copied()
        .collect();
    let mut patch_tiles = None;
    let atlas = !patched.is_empty() || !composite_pairs.is_empty();
    let mut atlas_over = false;
    if atlas {
        let (base, over, tiles) = patch_atlas(decoded, &patched, &palette, &composite_pairs)?;
        index_writes(&mut map_effects, &tiles)?;
        index_overworld_patches(&mut overworld_patches, &tiles)?;
        index_overworld_patches(&mut chunk_def_copies, &tiles)?;
        let base_name = format!("maps/{stem}_patch.png");
        let over_name = over
            .is_some()
            .then(|| format!("maps/{stem}_patch_over.png"));
        patch_tiles = Some(atlas_json(
            tiles,
            &base_name,
            &base,
            over_name.as_deref(),
            over.as_deref(),
        )?);
        files.push((base_name, base));
        atlas_over = over.is_some();
        if let (Some(name), Some(image)) = (over_name, over) {
            files.push((name, image));
        }
    }

    let mut update_images = None;
    if needs_index_images(programs) {
        let mut names = Vec::with_capacity(files.len());
        let mut added = Vec::with_capacity(files.len());
        for (name, data) in &files {
            let target = format!("{}_indices.png", name.trim_end_matches(".png"));
            added.push((target.clone(), png::index_png(data)?));
            names.push((name.clone(), target));
        }
        files.extend(added);
        update_images = Some(names);
    }
    Ok(Pictures {
        files,
        map_effects,
        patch_counts,
        overworld_patches,
        chunk_def_copies,
        scene_chunks,
        variants,
        patch_tiles,
        update_images,
        art_cycle_images,
        priority,
        has_overlay,
        patched_chunks: patched.len(),
        atlas,
        atlas_over,
    })
}

/// Everything every map's pictures read.
pub struct MapInputs {
    pub maps: ExtractedMaps,
    pub effects: MapEffects,
    pub updates: MapUpdates,
}

/// Read the records and programs once.
pub fn map_inputs(rom: &[u8], decodes: &Decodes) -> Result<MapInputs, PackError> {
    let maps = extract_maps(rom, decodes)?;
    let effects = extract_map_effects(rom, maps.real())?;
    let updates = extract_map_updates(rom, maps.real())?;
    Ok(MapInputs {
        maps,
        effects,
        updates,
    })
}

/// The section: sheets in map order, then every real map, on one thread per
/// core. A failure stops further maps from starting and is returned once the
/// running ones end.
pub fn build(rom: &Rom, root: &Path) -> Result<crate::json::Json, PackError> {
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    let inputs = map_inputs(data, &decodes)?;
    let field = field_sprites(data, &decodes, &inputs)?;
    let written = write_sprites(data, &decodes, root, &field)?;
    let records: Vec<(&MapRecord, &MapSprites)> = inputs.maps.real().zip(&field.maps).collect();
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let failure = Mutex::new(None);
    let shop_maps = Mutex::new(Vec::new());
    let workers = crate::default_workers().clamp(1, records.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let Some(&(record, sprites)) =
                        records.get(next.fetch_add(1, Ordering::Relaxed))
                    else {
                        break;
                    };
                    match write_map(data, &decodes, root, record, sprites, &inputs) {
                        Ok(map) => shop_maps
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .push(map),
                        Err(error) => {
                            stop.store(true, Ordering::Relaxed);
                            failure
                                .lock()
                                .unwrap_or_else(PoisonError::into_inner)
                                .get_or_insert(error);
                        }
                    }
                }
            });
        }
    });
    if let Some(error) = failure.into_inner().unwrap_or_else(PoisonError::into_inner) {
        return Err(error);
    }
    // A counter names its shopkeeper by position, so the shops follow the maps.
    let mut shop_maps = shop_maps
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner);
    shop_maps.sort_by_key(|(map, _)| map.id);
    let (shop_maps, parts): (Vec<ShopMap>, Vec<MapPart>) = shop_maps.into_iter().unzip();
    let shops = crate::shops::build(rom, root, &shop_maps)?;
    manifest::fragment(
        data,
        root,
        parts,
        manifest::Inputs {
            maps: &inputs.maps,
            field: &field,
            sprites: written,
            effects_census: &inputs.effects.census,
            updates_census: &inputs.updates.census,
            shops,
        },
    )
}

/// One map's pictures and file; what the shops need from it.
fn write_map(
    data: &[u8],
    decodes: &Decodes,
    root: &Path,
    record: &MapRecord,
    sprites: &MapSprites,
    inputs: &MapInputs,
) -> Result<(ShopMap, MapPart), PackError> {
    let section = decode_map_section(data, decodes, record)?;
    let pictures = map_pictures(
        data,
        decodes,
        record,
        &section,
        &inputs.effects,
        &sprites.effects,
        &inputs.updates,
    )?;
    for (name, image) in &pictures.files {
        crate::write_file(root, name, image)?;
    }
    let programs = inputs
        .updates
        .per_map
        .get(&record.id)
        .map_or(&[][..], Vec::as_slice);
    let (file, anomalies, typed) = map_file(data, record, &section, &pictures, sprites, programs)?;
    let sha = crate::write_json(root, &format!("maps/{}.json", map_stem(record)), &file)?;
    let part = manifest::map_part(record, &section, &pictures, &file, sha, anomalies)?;
    Ok((
        ShopMap {
            id: typed.id.0,
            npcs: typed.npcs,
            collision: typed.collision.grid,
        },
        part,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_programs_that_rewrite_cram_need_index_images() {
        let program =
            |kind: &str| Json::object().with("program", Json::object().with("kind", kind));
        for kind in ["palette", "crystals", "edge", "edge_line"] {
            assert!(
                needs_index_images(&[program("noop"), program(kind)]),
                "{kind}"
            );
        }
        for kind in [
            "noop",
            "camera",
            "scroll",
            "flags",
            "tunnels",
            "music",
            "encounters",
            "unsupported",
        ] {
            assert!(!needs_index_images(&[program(kind)]), "{kind}");
        }
        assert!(!needs_index_images(&[]));
    }
}
