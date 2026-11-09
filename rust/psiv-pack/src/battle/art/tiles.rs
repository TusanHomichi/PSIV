//! The port of `psiv_tools/battle_tile_art.py`: the battle's command-window
//! and damage-number tiles, Nemesis streams the pack carries as CRAM-index
//! rows for the renderer to colour. That module cites the loaders.

use crate::PackError;
use crate::decode::nemesis;
use crate::json::Json;

/// `COMMAND_UI_STREAMS`: name, ROM offset, tile count, first VRAM tile.
const COMMAND_UI_STREAMS: [(&str, usize, usize, usize); 2] = [
    ("icons", 0x27D860, 30, 0x15E),
    ("cursor", 0x27DA70, 8, 0x17C),
];
/// `DAMAGE_DIGIT_STREAM`.
const DAMAGE_DIGIT_STREAM: (&str, usize, usize, usize) = ("digits", 0x27D778, 28, 0x5E4);
/// `loc_7634` copies fifteen words from `loc_76A4` into CRAM line 3.
const COMMAND_UI_PALETTE: usize = 0x76A4;
const COMMAND_UI_PALETTE_WORDS: usize = 15;

/// One tile's eight rows, one hex digit per pixel, as the VDP reads 4bpp.
fn rows(tile: &[u8]) -> Vec<String> {
    tile.chunks(4)
        .map(|row| {
            row.iter()
                .map(|byte| format!("{:X}{:X}", byte >> 4, byte & 15))
                .collect()
        })
        .collect()
}

/// `_stream_tiles`: a stream's description and its tiles.
fn stream_tiles(
    rom: &[u8],
    (name, offset, count, first): (&str, usize, usize, usize),
) -> Result<(Json, Vec<Json>), PackError> {
    let (data, consumed) = nemesis::decompress(rom, offset)?;
    if data.len() != count * 32 {
        return Err(PackError(format!(
            "battle art stream {name} at ${offset:X} holds {} tiles, expected {count}",
            data.len() / 32
        )));
    }
    let stream = Json::object()
        .with("name", name)
        .with("rom_offset", format!("0x{offset:X}"))
        .with("compressed_bytes", consumed)
        .with("tile_count", count)
        .with("first_vram_tile", first);
    let tiles = data
        .chunks(32)
        .enumerate()
        .map(|(index, tile)| {
            Json::object()
                .with("vram_tile", first + index)
                .with("rows", rows(tile))
        })
        .collect();
    Ok((stream, tiles))
}

/// `build_command_ui`.
pub fn command_ui(rom: &[u8]) -> Result<Json, PackError> {
    let mut streams = Vec::new();
    let mut tiles = Vec::new();
    for spec in COMMAND_UI_STREAMS {
        let (stream, more) = stream_tiles(rom, spec)?;
        streams.push(stream);
        tiles.extend(more);
    }
    let palette: Vec<u16> = rom
        .get(COMMAND_UI_PALETTE..COMMAND_UI_PALETTE + 2 * COMMAND_UI_PALETTE_WORDS)
        .ok_or_else(|| PackError("command art palette at $76A4 runs past the ROM".into()))?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect();
    Ok(Json::object()
        .with("kind", "battle_command_ui_art")
        .with(
            "source",
            "loc_7382 (ps4.asm:10978): NemDecomp to VRAM $2BC0 and $2F80",
        )
        .with("palette_line_3", palette)
        .with("streams", streams)
        .with("tile_count", tiles.len())
        .with("tiles", tiles))
}

/// `build_damage_digits`.
pub fn damage_digits(rom: &[u8]) -> Result<Json, PackError> {
    let (stream, tiles) = stream_tiles(rom, DAMAGE_DIGIT_STREAM)?;
    Ok(Json::object()
        .with("kind", "battle_damage_digit_art")
        .with("source", "ps4.asm:10007-10009: NemDecomp to VRAM $BC80")
        .with("streams", vec![stream])
        .with("tile_count", tiles.len())
        .with("tiles", tiles))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_row_is_its_pixels_high_nibble_first() {
        let mut tile = [0u8; 32];
        tile[0] = 0x1F;
        tile[3] = 0xA0;
        tile[31] = 0x05;
        let rows = rows(&tile);
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[0], "1F0000A0");
        assert_eq!(rows[7], "00000005");
    }
}
