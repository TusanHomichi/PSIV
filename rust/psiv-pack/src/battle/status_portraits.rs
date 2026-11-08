//! The eleven STATUS portraits: the port of `psiv_tools/status_portraits.py`.
//! Each is a Nemesis art blob drawn through its own 10x10 Plane mapping on
//! CRAM line 2, and the ages sit in their own word table.

use std::path::Path;

use crate::decode::nemesis;
use crate::json::Json;
use crate::sprites::field::pal_init_line_3;
use crate::{PackError, gfx, png};

const ART_TABLE: usize = 0x5D548;
const MAP_TABLE: usize = 0x5D574;
const AGE_TABLE: usize = 0x2A8E12;
const PORTRAITS: usize = 11;
const COLUMNS: usize = 10;
const CELLS: usize = COLUMNS * COLUMNS;
/// The VRAM tile the art is loaded at, which the mapping words name.
const ART_VRAM_TILE: u16 = 0x580;
const LINE: u16 = 2;

/// `character_age`: zero is the original unknown-age marker.
pub fn age(rom: &[u8], character: usize) -> Result<u16, PackError> {
    Ok(crate::m68k::w(rom, AGE_TABLE + character * 2)?)
}

/// `planes.compose` over one portrait's cells: each word's pattern, flipped as
/// it says, on its CRAM line; pattern index 0 shows the backdrop.
fn compose(words: &[u16], tiles: &[[u8; 64]]) -> Result<Vec<u8>, PackError> {
    let width = COLUMNS * 8;
    let mut pixels = vec![0u8; width * (words.len() / COLUMNS) * 8];
    for (index, &word) in words.iter().enumerate() {
        let tile = word & 0x07FF;
        let art = tile
            .checked_sub(ART_VRAM_TILE)
            .map(usize::from)
            .and_then(|i| tiles.get(i))
            .ok_or_else(|| {
                PackError(format!(
                    "Cell {index} references VRAM tile 0x{tile:03X}, which is outside the \
                     {}-pattern blob loaded at VRAM tile 0x{ART_VRAM_TILE:03X}",
                    tiles.len()
                ))
            })?;
        let (h_flip, v_flip) = (word & 0x0800 != 0, word & 0x1000 != 0);
        let line = ((word >> 13) & 3) as u8 * 16;
        let (ox, oy) = ((index % COLUMNS) * 8, (index / COLUMNS) * 8);
        for y in 0..8 {
            for x in 0..8 {
                let sx = if h_flip { 7 - x } else { x };
                let sy = if v_flip { 7 - y } else { y };
                let p = art[sy * 8 + sx];
                pixels[(oy + y) * width + ox + x] = if p == 0 { 0 } else { line + p };
            }
        }
    }
    Ok(pixels)
}

/// `emit_status_portraits`: write the eleven PNGs; each record goes on its
/// character as `status_portrait`.
pub fn write(rom: &[u8], root: &Path) -> Result<Vec<Json>, PackError> {
    let mut palette = vec![[0u8; 3]; 32];
    palette.extend(pal_init_line_3(rom)?);
    palette.extend([[0u8; 3]; 16]);
    (0..PORTRAITS)
        .map(|character| {
            let art = crate::m68k::l(rom, ART_TABLE + character * 4)? as usize;
            let mapping = crate::m68k::l(rom, MAP_TABLE + character * 4)? as usize;
            let (raw, consumed) = nemesis::decompress(rom, art)?;
            let words = (mapping..mapping + CELLS * 2)
                .step_by(2)
                .map(|at| crate::m68k::w(rom, at))
                .collect::<Result<Vec<_>, _>>()?;
            if words.iter().any(|word| (word >> 13) & 3 != LINE) {
                return Err(PackError(format!(
                    "STATUS portrait {character} uses an unexpected palette"
                )));
            }
            let pixels = compose(&words, &gfx::decode_tiles(&raw)?)?;
            let (width, height) = (COLUMNS * 8, CELLS / COLUMNS * 8);
            let image = png::encode_indexed(width as u32, height as u32, &pixels, &palette, &[0])?;
            let relative = format!("battle/status_portraits/{character:02x}.png");
            crate::write_file(root, &relative, &image)?;
            Ok(Json::object()
                .with("png", relative)
                .with("png_sha256", png::image_sha256(&image)?)
                .with("art", format!("0x{art:06X}"))
                .with("mapping", format!("0x{mapping:06X}"))
                .with("source_consumed", consumed)
                .with("size_pixels", vec![width, height])
                .with("screen_pixels", vec![24u8, 16])
                .with("cram_line", LINE))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_lands_on_its_line_flipped_and_zero_stays_backdrop() {
        let mut tile = [0u8; 64];
        tile[0] = 5;
        tile[7] = 1;
        let mut words = vec![0x4580u16; CELLS];
        words[1] = 0x4580 | 0x0800;
        let pixels = compose(&words, &[tile]).unwrap();
        let width = COLUMNS * 8;
        assert_eq!(pixels[0], 37);
        assert_eq!(pixels[7], 33);
        assert_eq!(pixels[1], 0);
        assert_eq!((pixels[8], pixels[15]), (33, 37));
        assert_eq!(pixels.len(), width * 80);
    }

    #[test]
    fn a_cell_outside_the_blob_refuses() {
        let words = vec![0x4581u16; CELLS];
        assert!(compose(&words, &[[0; 64]]).is_err());
        let below = vec![0x457Fu16; CELLS];
        assert!(compose(&below, &[[0; 64]]).is_err());
    }
}
