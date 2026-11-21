//! Tile-DMA art-cycle frames for Garuberk Tower eyes (`#101`).
//!
//! `Event_GaruberkTwEyeAction1` (`$06F8A4`, `ps4.asm:148951-149019`) and
//! `MapUpdate_GaruberkTowerPart4` (`$055832`, `ps4.asm:114142-114191`) both
//! ping-pong three `$9E0`-byte (79-tile) Kosinski frames into VRAM `$4060`
//! (tile `$0203`). Frame 0 is the map's own base upload at tile `$0203`.

use std::collections::BTreeMap;

use super::draw::{priority_overlay, render_layout};
use super::effects::{field, text};
use super::layouts::{MapLayout, TilePatterns};
use super::records::MapRecord;
use crate::PackError;
use crate::decode::{Call, Decodes};
use crate::gfx;
use crate::json::Json;
use crate::m68k::{l, w};

/// VRAM destination `$4060` (`move.w #$4060, d1`) divided by 32 bytes/tile.
pub const EYE_FIRST_TILE: usize = 0x0203;
/// Each Kosinski blob (`$1C7460`/`$1C79F0`/`$1C7FE0`) decompresses to 78 4bpp
/// tiles (`$9C0` = 2496 bytes) inside its `$9E0`-byte (`$4F0`-word) RAM slot.
pub const EYE_TILE_COUNT: usize = 78;
const EYE_FRAME_BYTES: usize = EYE_TILE_COUNT * gfx::TILE_SIZE;
const EYE_ACTION_1: usize = 0x06F8A4;
const EYE_ACTION_1_LOOP: usize = 0x06F91E;
const KOS_DECOMP: u32 = 0x0004_1BEA;
const GARUBERK_PART2_MAP_ID: usize = 0x19A;

/// Decoded 8x8 tile frames ready to patch into a map's VRAM image.
pub struct TileDmaFrames {
    pub first_tile: usize,
    pub frames: Vec<Vec<[u8; 64]>>,
}

fn event_eye_sources(rom: &[u8]) -> Result<[usize; 3], PackError> {
    let expected_ram: [u32; 3] = [0xFFFF_0000, 0xFFFF_09E0, 0xFFFF_13C0];
    let mut sources = [0usize; 3];
    for (i, &ram) in expected_ram.iter().enumerate() {
        let at = EYE_ACTION_1 + i * 18;
        if w(rom, at)? != 0x41F9
            || w(rom, at + 6)? != 0x43F9
            || l(rom, at + 8)? != ram
            || w(rom, at + 12)? != 0x4EB9
            || l(rom, at + 14)? != KOS_DECOMP
        {
            return Err(PackError(format!(
                "Event_GaruberkTwEyeAction1 KosDecomp triple {i} at {at:#08x} moved"
            )));
        }
        sources[i] = l(rom, at + 2)? as usize;
    }
    // `loc_6F91E` (`ps4.asm:148977-149019`): pin VRAM destination `$4060` and
    // transfer length `$4F0` words at the DMA call tail (`$06F986`, `$06F98A`).
    if w(rom, EYE_ACTION_1_LOOP + 0x68)? != 0x323C
        || w(rom, EYE_ACTION_1_LOOP + 0x6A)? != 0x4060
        || w(rom, EYE_ACTION_1_LOOP + 0x6C)? != 0x343C
        || w(rom, EYE_ACTION_1_LOOP + 0x6E)? != 0x04F0
    {
        return Err(PackError(
            "Event_GaruberkTwEyeAction1 DMA destination or word count moved".into(),
        ));
    }
    Ok(sources)
}

fn record_ram_sources(record: &MapRecord) -> Result<[usize; 3], PackError> {
    let entries = &record.sprite_data.entries;
    let expected_words: [u16; 3] = [0x0000, 0x09E0, 0x13C0];
    if entries.len() != 3
        || entries
            .iter()
            .zip(expected_words)
            .any(|(entry, word)| entry.word != word)
    {
        return Err(PackError(format!(
            "map 0x{:03X}: expected 3 RAM art frames at 0x0000/0x09E0/0x13C0",
            record.id
        )));
    }
    Ok([entries[0].source, entries[1].source, entries[2].source])
}

/// Extract tile-DMA art-cycle frames for maps that run
/// `MapUpdate_GaruberkTowerPart4` (`$22`) or `Event_GaruberkTwEyeAction1`.
pub fn extract_tile_dma_frames(
    rom: &[u8],
    decodes: &Decodes,
    record: &MapRecord,
    programs: &[Json],
    base_patterns: &TilePatterns,
) -> Result<Option<TileDmaFrames>, PackError> {
    let has_update = programs.iter().any(|entry| {
        field(entry, "program").is_some_and(|program| text(program, "kind") == "tile_dma_cycle")
    });
    let sources = if has_update {
        record_ram_sources(record)?
    } else if record.id == GARUBERK_PART2_MAP_ID {
        event_eye_sources(rom)?
    } else {
        return Ok(None);
    };
    let mut frames = Vec::with_capacity(sources.len());
    for (idx, offset) in sources.into_iter().enumerate() {
        let decoded = decodes.get(Call::Kosinski { offset })?;
        let raw = decoded.bytes();
        if raw.len() != EYE_FRAME_BYTES {
            return Err(PackError(format!(
                "map 0x{:03X}: tile-DMA frame {idx} at 0x{offset:06X} is {} bytes, expected {EYE_FRAME_BYTES}",
                record.id,
                raw.len()
            )));
        }
        let tiles = raw
            .as_chunks::<{ gfx::TILE_SIZE }>()
            .0
            .iter()
            .map(|pattern| gfx::decode_tile(pattern))
            .collect::<Result<Vec<[u8; 64]>, _>>()?;
        frames.push(tiles);
    }
    // Frame 0 must match the tiles the map itself loaded at VRAM tile `$203`.
    let base_slice = &base_patterns.tiles[EYE_FIRST_TILE..EYE_FIRST_TILE + EYE_TILE_COUNT];
    if frames[0].as_slice() != base_slice {
        return Err(PackError(format!(
            "map 0x{:03X}: tile-DMA frame 0 does not match base VRAM tiles 0x{EYE_FIRST_TILE:03X}..0x{:03X}",
            record.id,
            EYE_FIRST_TILE + EYE_TILE_COUNT
        )));
    }
    Ok(Some(TileDmaFrames {
        first_tile: EYE_FIRST_TILE,
        frames,
    }))
}

/// Render every non-base frame of `dma` for one layout (base map or layout
/// variant) and its optional priority overlay. Frames identical to frame 0
/// reuse frame 0's path without writing a duplicate PNG.
#[allow(clippy::too_many_arguments)]
pub fn render_art_cycle_frames(
    decoded: &MapLayout,
    palette: &[[u8; 3]],
    dma: &TileDmaFrames,
    base_png_name: &str,
    base_png: &[u8],
    over_png_name: Option<&str>,
    over_png: Option<&[u8]>,
    files: &mut Vec<(String, Vec<u8>)>,
    art_cycle_images: &mut BTreeMap<String, Vec<String>>,
) -> Result<(), PackError> {
    let mut base_list = vec![base_png_name.to_owned()];
    let mut over_list = over_png_name.map(|name| vec![name.to_owned()]);
    for (idx, frame_tiles) in dma.frames.iter().enumerate().skip(1) {
        let mut patterns = decoded.patterns.clone();
        for (i, tile) in frame_tiles.iter().enumerate() {
            patterns.tiles[dma.first_tile + i] = *tile;
            patterns.loaded[dma.first_tile + i] = true;
        }
        let image = render_layout(
            &decoded.chunks,
            &decoded.bg,
            &decoded.fg,
            &patterns,
            palette,
        )?;
        if image == base_png {
            base_list.push(base_png_name.to_owned());
        } else {
            let name = format!("{}_art{idx}.png", base_png_name.trim_end_matches(".png"));
            files.push((name.clone(), image));
            base_list.push(name);
        }
        if let (Some(over_name), Some(over_base), Some(over_out)) =
            (over_png_name, over_png, over_list.as_mut())
        {
            let patched = MapLayout {
                spec: decoded.spec.clone(),
                chunks: decoded.chunks.clone(),
                fg: decoded.fg.clone(),
                bg: decoded.bg.clone(),
                collision: decoded.collision.clone(),
                patterns,
            };
            let (overlay, _) = priority_overlay(&patched, palette)?;
            if overlay.as_deref() == Some(over_base) {
                over_out.push(over_name.to_owned());
            } else if let Some(overlay) = overlay {
                let name = format!("{}_art{idx}.png", over_name.trim_end_matches(".png"));
                files.push((name.clone(), overlay));
                over_out.push(name);
            } else {
                over_out.push(over_name.to_owned());
            }
        }
    }
    if base_list.iter().any(|path| path != base_png_name) {
        art_cycle_images.insert(base_png_name.to_owned(), base_list);
    }
    if let (Some(over_name), Some(over_out)) = (over_png_name, over_list)
        && over_out.iter().any(|path| path != over_name)
    {
        art_cycle_images.insert(over_name.to_owned(), over_out);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps::overworld::decode_map_section;
    use crate::maps::pictures::{map_inputs, map_pictures};

    #[test]
    fn garuberk_eye_maps_emit_distinct_art_cycle_frames_and_reject_drifted_rom() {
        let Some(rom) = crate::test_support::retail_rom("garuberk eye art cycle") else {
            return;
        };
        let data = rom.bytes();
        let decodes = Decodes::new(data);
        let inputs = map_inputs(data, &decodes).unwrap();

        for id in [0x19A, 0x19C, 0x19D, 0x19E, 0x19F] {
            let record = inputs.maps.real().find(|m| m.id == id).unwrap();
            let section = decode_map_section(data, &decodes, record).unwrap();
            let pictures = map_pictures(
                data,
                &decodes,
                record,
                &section,
                &inputs.effects,
                &[],
                &inputs.updates,
            )
            .unwrap();
            let base_png = format!("maps/{}.png", crate::maps::pictures::map_stem(record));
            let frames = pictures
                .art_cycle_images
                .get(&base_png)
                .unwrap_or_else(|| panic!("map {id:#05x} missing art_cycle_images for {base_png}"));
            assert_eq!(frames.len(), 3, "map {id:#05x}");
            assert_eq!(frames[0], base_png);
            assert_ne!(frames[1], base_png, "map {id:#05x} frame 1 must differ");
            assert_ne!(frames[2], base_png, "map {id:#05x} frame 2 must differ");
            // Maps with palette updates also receive index PNGs for each art frame.
            if let Some(indexed) = &pictures.update_images {
                let keys: std::collections::BTreeSet<&str> =
                    indexed.iter().map(|(k, _)| k.as_str()).collect();
                assert!(
                    keys.contains(frames[1].as_str()),
                    "map {id:#05x} art1 index"
                );
                assert!(
                    keys.contains(frames[2].as_str()),
                    "map {id:#05x} art2 index"
                );
            }
        }

        // Unrelated map control: Piata emits no art_cycle_images.
        let piata = inputs.maps.real().find(|m| m.id == 0x10).unwrap();
        let section = decode_map_section(data, &decodes, piata).unwrap();
        let pictures = map_pictures(
            data,
            &decodes,
            piata,
            &section,
            &inputs.effects,
            &[],
            &inputs.updates,
        )
        .unwrap();
        assert!(pictures.art_cycle_images.is_empty());

        // Negative control: corrupting Event_GaruberkTwEyeAction1's VRAM dest
        // `$4060` must refuse extraction.
        let mut broken = data.to_vec();
        broken[EYE_ACTION_1_LOOP + 0x6A..EYE_ACTION_1_LOOP + 0x6C]
            .copy_from_slice(&0x4080u16.to_be_bytes());
        assert!(
            event_eye_sources(&broken)
                .unwrap_err()
                .0
                .contains("DMA destination")
        );
    }
}
