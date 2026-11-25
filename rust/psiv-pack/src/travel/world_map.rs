//! Overworld planet map extraction (`FieldRoutine_WorldMap`, `ps4.asm:136977-137245`, #122).
//!
//! Retail reads sixteen `$400`-byte chunk strips per planet (`loc_10BE02` for Motavia,
//! `loc_1175C4` for Dezolis) and maps each chunk id through a colour byte table
//! (`loc_66986` for Motavia on CRAM line 1, `loc_66A5A` for Dezolis on CRAM line 0) into a
//! 2x2-pixel block. Retail Bug #6 (`docs/source-notes/README.md`) starts at the 64-byte
//! strip-pointer table itself; this extractor dereferences the strip pointers so the
//! rendered images show the actual Motavia (256x256) and Dezolis (256x128) continents.

use std::path::Path;

use psiv_data::WorldMapAsset;

use crate::PackError;
use crate::decode::Decodes;
use crate::maps::layouts::map_palette_rgb;
use crate::maps::records::walk_map;
use crate::png;
use crate::text::dialogue_tree_specs;

/// `FieldMapPtrs` base offset in the US retail ROM.
const FIELD_MAP_PTRS: usize = 0x10_0000;
/// Width of each overworld strip in chunks (128 chunks = 256 pixels).
const CHUNKS_WIDE: usize = 128;
/// Height of one `$400`-byte strip in chunks (8 rows x 128 cols = 1024 bytes).
const STRIP_ROWS: usize = 8;
const STRIP_BYTES: usize = CHUNKS_WIDE * STRIP_ROWS;

/// Instruction-pointer guards in `FieldRoutine_WorldMap` (`ps4.asm:137006-137029`),
/// pinned by `(offset, length, sha256)` (#461).
const WORLD_MAP_GUARDS: [(usize, usize, &str); 4] = [
    (
        0x06_66CE,
        4,
        "d7f57e29536fb17df665bd65cf12a36dc19a6c0d2dc011c192921a54a22992f6",
    ),
    (
        0x06_66DC,
        4,
        "1addd9ffaf62f7ed718d29d8f5b8c94814b20de38e2fc98aa7f95ae83b3d3bde",
    ),
    (
        0x06_6714,
        4,
        "713b50dfc5f236ff9386e697a1fad74508a79d0f794bc8cad5c9629c6908e61f",
    ),
    (
        0x06_6722,
        4,
        "66376597460e124bea47e87344b0d4da095fd03539fa2bd2d68faee6a53c99c1",
    ),
];

struct PlanetSpec {
    world: u8,
    map_id: usize,
    name: &'static str,
    png: &'static str,
    strip_table: usize,
    strips: usize,
    color_table: usize,
    palette_line: usize,
    scroll_y: bool,
    initial_y: i16,
}

const PLANETS: [PlanetSpec; 2] = [
    PlanetSpec {
        world: 0,
        map_id: 0x000,
        name: "MOTAVIA",
        png: "travel/world_map_motavia.png",
        strip_table: 0x10_BE02,
        strips: 16,
        color_table: 0x06_6986,
        palette_line: 1,
        scroll_y: true,
        initial_y: 0,
    },
    PlanetSpec {
        world: 1,
        map_id: 0x001,
        name: "DEZOLIS",
        png: "travel/world_map_dezolis.png",
        strip_table: 0x11_75C4,
        strips: 8,
        color_table: 0x06_6A5A,
        palette_line: 0,
        scroll_y: false,
        initial_y: -48,
    },
];

fn be32(rom: &[u8], at: usize) -> Result<usize, PackError> {
    let bytes = rom
        .get(at..at + 4)
        .ok_or_else(|| PackError(format!("world map pointer at 0x{at:06X} runs past the ROM")))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
}

fn check_guards(rom: &[u8]) -> Result<(), PackError> {
    for (offset, length, digest) in WORLD_MAP_GUARDS {
        if !crate::rom::matches_pin(rom, offset, (length, digest)) {
            return Err(PackError(format!(
                "world map guard failed at 0x{offset:06X}"
            )));
        }
    }
    Ok(())
}

fn render_planet(
    rom: &[u8],
    spec: &PlanetSpec,
    trees: &[(usize, usize)],
) -> Result<(Vec<u8>, WorldMapAsset), PackError> {
    let map_ptr = be32(rom, FIELD_MAP_PTRS + spec.map_id * 4)?;
    let record = walk_map(rom, spec.map_id, map_ptr, trees)?;
    let rgb = map_palette_rgb(rom, record.palette.pointer)?;
    let start = spec.palette_line * 16;
    let palette = rgb.get(start..start + 16).ok_or_else(|| {
        PackError(format!(
            "world map {} palette line {} out of range",
            spec.name, spec.palette_line
        ))
    })?;

    let width = CHUNKS_WIDE * 2;
    let height = spec.strips * STRIP_ROWS * 2;
    let mut pixels = vec![0u8; width * height];

    for strip_index in 0..spec.strips {
        let strip_ptr = be32(rom, spec.strip_table + strip_index * 4)?;
        let strip = rom.get(strip_ptr..strip_ptr + STRIP_BYTES).ok_or_else(|| {
            PackError(format!(
                "world map {} strip {strip_index} at 0x{strip_ptr:06X} runs past the ROM",
                spec.name
            ))
        })?;
        for row in 0..STRIP_ROWS {
            let py = (strip_index * STRIP_ROWS + row) * 2;
            let row0 = py * width;
            let row1 = (py + 1) * width;
            for col in 0..CHUNKS_WIDE {
                let chunk = usize::from(strip[row * CHUNKS_WIDE + col]);
                let color_byte = *rom.get(spec.color_table + chunk).ok_or_else(|| {
                    PackError(format!(
                        "world map {} color lookup 0x{:06X} runs past the ROM",
                        spec.name,
                        spec.color_table + chunk
                    ))
                })?;
                let left = (color_byte >> 4) & 0x0F;
                let right = color_byte & 0x0F;
                let px = col * 2;
                pixels[row0 + px] = left;
                pixels[row0 + px + 1] = right;
                pixels[row1 + px] = left;
                pixels[row1 + px + 1] = right;
            }
        }
    }

    let png_bytes = png::encode_indexed(width as u32, height as u32, &pixels, palette, &[])?;
    let asset = WorldMapAsset {
        world: spec.world,
        name: spec.name.to_owned(),
        png: spec.png.to_owned(),
        width: width as u16,
        height: height as u16,
        scroll_y: spec.scroll_y,
        initial_y: spec.initial_y,
    };
    Ok((png_bytes, asset))
}

/// Decodes and writes the Motavia and Dezolis world-map PNGs under `travel/`.
pub fn extract_and_write(rom: &[u8], root: &Path) -> Result<Vec<WorldMapAsset>, PackError> {
    check_guards(rom)?;
    let decodes = Decodes::new(rom);
    let trees: Vec<(usize, usize)> = dialogue_tree_specs(rom, &decodes)?
        .iter()
        .map(|spec| (spec.start, spec.tree))
        .collect();
    let mut assets = Vec::with_capacity(PLANETS.len());
    for spec in &PLANETS {
        let (image, asset) = render_planet(rom, spec, &trees)?;
        crate::write_file(root, &asset.png, &image)?;
        assets.push(asset);
    }
    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_map_guards_reject_shifted_pointers() {
        let err = check_guards(&[0; 16]).unwrap_err();
        assert!(
            err.0.contains("world map guard failed at 0x0666CE"),
            "{err}"
        );
    }

    #[test]
    fn retail_world_maps_render_motavia_and_dezolis() {
        use crate::test_support::{retail_rom, scratch};
        let Some(rom) = retail_rom("world map retail test") else {
            return;
        };
        let root = scratch("world_map");
        let assets = extract_and_write(rom.bytes(), &root).unwrap();
        assert_eq!(assets.len(), 2);
        assert_eq!(
            (
                assets[0].world,
                assets[0].width,
                assets[0].height,
                assets[0].scroll_y,
                assets[0].initial_y
            ),
            (0, 256, 256, true, 0)
        );
        assert_eq!(
            (
                assets[1].world,
                assets[1].width,
                assets[1].height,
                assets[1].scroll_y,
                assets[1].initial_y
            ),
            (1, 256, 128, false, -48)
        );
        for asset in &assets {
            let bytes = std::fs::read(root.join(&asset.png)).unwrap();
            assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
            let decoded = crate::png::decode(&bytes).unwrap();
            assert_eq!(decoded.width, usize::from(asset.width));
            assert_eq!(decoded.height, usize::from(asset.height));
            let unique_colours: std::collections::BTreeSet<_> =
                decoded.pixels.iter().copied().collect();
            assert!(
                unique_colours.len() >= 4,
                "{} should have varied terrain",
                asset.name
            );
        }
        std::fs::remove_dir_all(&root).unwrap();

        let mut broken = rom.bytes().to_vec();
        broken[0x06_66CE] ^= 1;
        assert!(check_guards(&broken).unwrap_err().0.contains("0x0666CE"));
    }
}
