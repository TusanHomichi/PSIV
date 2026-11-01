//! The `map_pictures` section: every PNG under `maps/`, built from the ROM map
//! by map, in parallel. One map's work is [`map_pictures`], the part of
//! `psiv_tools/pack.py`'s map loop that makes pictures; its JSON goes to the map
//! file once the sprites are ported (slice 4), and until then the gate holds it
//! to Python as part of the `layouts` probe.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use sha2::{Digest, Sha256};

use super::draw::{priority_overlay, render_layout};
use super::effects::{MapEffects, extract_map_effects, field, int, text};
use super::layouts::{
    Layout, MapLayout, Plane, decode_collision, map_palette_rgb, unloaded_patterns,
};
use super::overworld::decode_map_section;
use super::patches::{
    PatchCounts, atlas_json, index_overworld_patches, index_writes, patch_atlas,
    resolve_map_effects, resolve_overworld_patches, scene_patch_chunks,
};
use super::records::MapRecord;
use super::updates::{MapUpdates, extract_map_updates};
use super::{ExtractedMaps, extract_maps};
use crate::decode::{Call, Decodes};
use crate::json::Json;
use crate::png::{self, hex};
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
    files: &mut Vec<(String, Vec<u8>)>,
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
    files.push((png_name, image));
    if let (Some(name), Some(image)) = (over_name, overlay) {
        files.push((name, image));
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
    pub scene_chunks: Vec<u8>,
    pub variants: Vec<Json>,
    pub patch_tiles: Option<Json>,
    /// `map_update_images`, when an update needs palette-index renders.
    pub update_images: Option<Vec<(String, String)>>,
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
/// last: they are every other picture with its palette rewritten.
pub fn map_pictures(
    rom: &[u8],
    decodes: &Decodes,
    record: &MapRecord,
    effects: &MapEffects,
    updates: &MapUpdates,
) -> Result<Pictures, PackError> {
    let section = decode_map_section(rom, decodes, record)?;
    let decoded = section.layout();
    let mut palette = map_palette_rgb(rom, decoded.spec.palette)?;
    palette.truncate(32);
    let stem = map_stem(record);
    let image = render_layout(
        &decoded.chunks,
        &decoded.bg,
        &decoded.fg,
        &decoded.patterns,
        &palette,
    )?;
    let (overlay, _) = priority_overlay(decoded, &palette)?;
    let mut files = vec![(format!("maps/{stem}.png"), image)];
    let has_overlay = overlay.is_some();
    if let Some(overlay) = overlay {
        files.push((format!("maps/{stem}_over.png"), overlay));
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
        &mut files,
    )?;

    let entries = effects
        .per_map
        .get(&record.id)
        .map_or(&[][..], Vec::as_slice);
    let (mut map_effects, written, patch_counts) = resolve_map_effects(decoded, entries)?;
    let (mut overworld_patches, composite_pairs) =
        resolve_overworld_patches(decoded, section.overworld())?;
    if !composite_pairs.is_empty() && !has_overlay {
        return Err(PackError(format!(
            "map 0x{:03X}: overworld hook needs a base priority overlay",
            record.id
        )));
    }
    let scene_chunks = scene_patch_chunks(rom, record)?;
    let patched: BTreeSet<u8> = written.iter().chain(&scene_chunks).copied().collect();
    let mut patch_tiles = None;
    if !patched.is_empty() || !composite_pairs.is_empty() {
        let (base, over, tiles) = patch_atlas(decoded, &patched, &palette, &composite_pairs)?;
        index_writes(&mut map_effects, &tiles)?;
        index_overworld_patches(&mut overworld_patches, &tiles)?;
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
        if let (Some(name), Some(image)) = (over_name, over) {
            files.push((name, image));
        }
    }

    let programs = updates
        .per_map
        .get(&record.id)
        .map_or(&[][..], Vec::as_slice);
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
        scene_chunks,
        variants,
        patch_tiles,
        update_images,
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

/// The section: every real map, on one thread per core. A failure stops further
/// maps from starting and is returned once the running ones end.
pub fn build(rom: &Rom, root: &Path) -> Result<(), PackError> {
    let data = rom.bytes();
    let decodes = Decodes::new(data);
    let inputs = map_inputs(data, &decodes)?;
    let records: Vec<&MapRecord> = inputs.maps.real().collect();
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let failure = Mutex::new(None);
    let workers = crate::default_workers().clamp(1, records.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let Some(record) = records.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        break;
                    };
                    let result =
                        map_pictures(data, &decodes, record, &inputs.effects, &inputs.updates)
                            .and_then(|pictures| {
                                pictures.files.iter().try_for_each(|(name, image)| {
                                    crate::write_file(root, name, image)
                                })
                            });
                    if let Err(error) = result {
                        stop.store(true, Ordering::Relaxed);
                        failure
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .get_or_insert(error);
                    }
                }
            });
        }
    });
    failure
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner)
        .map_or(Ok(()), Err)
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
