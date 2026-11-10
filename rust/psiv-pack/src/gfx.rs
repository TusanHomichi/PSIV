//! Colours and tiles: the primitives of `psiv_tools/gfx.py`, which owns the
//! receipts. A CRAM word is `%0000BBB0GGG0RRR0`; its 3-bit levels widen to
//! 8 bits through the GPGX/RGB565 ramp, the one palette choke point. A
//! pattern is 32 bytes, two pixels a byte, high nybble first.

use crate::json::Json;
use crate::png::{self, PngError};

pub const TILE_SIZE: usize = 32;
pub const TILE_WIDTH: usize = 8;
pub const TILE_HEIGHT: usize = 8;
pub const COLORS_PER_LINE: usize = 16;
/// The bits of a CRAM word the VDP reads.
pub const CRAM_DEFINED_MASK: u16 = 0x0EEE;

/// `GPGX_RGB565_RAMP`: each channel's eight levels (`psiv_data::color`).
pub use psiv_data::color::{RAMP_B, RAMP_G, RAMP_R};

/// `GRAYSCALE_RAMP`: the fallback for art whose palette line is not pinned.
pub const GRAYSCALE_RAMP: [[u8; 3]; 16] = {
    let mut ramp = [[0u8; 3]; 16];
    let mut i = 0;
    while i < 16 {
        let v = (i * 17) as u8;
        ramp[i] = [v, v, v];
        i += 1;
    }
    ramp
};

/// A picture the primitives refuse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphicsError(pub String);

impl std::fmt::Display for GraphicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GraphicsError {}

impl From<GraphicsError> for crate::PackError {
    fn from(error: GraphicsError) -> Self {
        crate::PackError(error.0)
    }
}

impl From<PngError> for GraphicsError {
    fn from(error: PngError) -> Self {
        GraphicsError(error.0)
    }
}

/// One decoded CRAM word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub raw: u16,
    /// Red, green and blue levels, 0..7.
    pub levels: [u8; 3],
}

impl Color {
    /// `decode_color`.
    pub fn decode(word: u16) -> Self {
        Color {
            raw: word,
            levels: [
                ((word >> 1) & 7) as u8,
                ((word >> 5) & 7) as u8,
                ((word >> 9) & 7) as u8,
            ],
        }
    }

    /// The colour widened through the ramp (`palette_rgb` of one entry).
    pub fn rgb(&self) -> [u8; 3] {
        let [r, g, b] = self.levels.map(usize::from);
        [RAMP_R[r], RAMP_G[g], RAMP_B[b]]
    }

    /// The record `decode_color` returns.
    pub fn to_json(&self) -> Json {
        let rgb = self.rgb();
        let mut entry = Json::object()
            .with("raw", format!("0x{:04X}", self.raw))
            .with(
                "levels",
                Json::object()
                    .with("r", self.levels[0])
                    .with("g", self.levels[1])
                    .with("b", self.levels[2]),
            )
            .with("rgb", rgb.to_vec())
            .with(
                "hex",
                format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
            );
        if self.raw & !CRAM_DEFINED_MASK != 0 {
            // The VDP ignores these bits; a set one usually means a wrong offset.
            entry.insert("unused_bits_set", true);
        }
        entry
    }
}

/// `expand_channel`: one level through one channel's ramp.
pub fn expand_channel(level: u8, channel: char) -> Result<u8, GraphicsError> {
    if level > 7 {
        return Err(GraphicsError(format!(
            "CRAM channel level {level} is outside 0..7"
        )));
    }
    let ramp = match channel {
        'r' => &RAMP_R,
        'g' => &RAMP_G,
        'b' => &RAMP_B,
        other => {
            return Err(GraphicsError(format!("unknown CRAM channel '{other}'")));
        }
    };
    Ok(ramp[usize::from(level)])
}

/// `decode_palette`: a run of big-endian CRAM words.
pub fn decode_palette(raw: &[u8]) -> Result<Vec<Color>, GraphicsError> {
    if !raw.len().is_multiple_of(2) {
        return Err(GraphicsError(format!(
            "Palette data is {} bytes, which is not a whole number of words",
            raw.len()
        )));
    }
    Ok(raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| Color::decode(u16::from_be_bytes(pair)))
        .collect())
}

/// `palette_rgb`.
pub fn palette_rgb(colors: &[Color]) -> Vec<[u8; 3]> {
    colors.iter().map(Color::rgb).collect()
}

/// `decode_tile`: one pattern into 64 palette indices, row-major.
pub fn decode_tile(chunk: &[u8]) -> Result<[u8; 64], GraphicsError> {
    if chunk.len() != TILE_SIZE {
        return Err(GraphicsError(format!(
            "A pattern is {TILE_SIZE} bytes, got {}",
            chunk.len()
        )));
    }
    let mut out = [0u8; 64];
    for (i, byte) in chunk.iter().enumerate() {
        out[2 * i] = byte >> 4;
        out[2 * i + 1] = byte & 0x0F;
    }
    Ok(out)
}

/// `decode_tiles`.
pub fn decode_tiles(data: &[u8]) -> Result<Vec<[u8; 64]>, GraphicsError> {
    if !data.len().is_multiple_of(TILE_SIZE) {
        return Err(GraphicsError(format!(
            "Art is {} bytes, which is not a whole number of {TILE_SIZE}-byte patterns",
            data.len()
        )));
    }
    data.as_chunks::<TILE_SIZE>()
        .0
        .iter()
        .map(|chunk| decode_tile(chunk))
        .collect()
}

/// `compose_sheet`: tiles left to right, top to bottom; `(w, h, indices)`.
pub fn compose_sheet(
    tiles: &[[u8; 64]],
    columns: usize,
) -> Result<(usize, usize, Vec<u8>), GraphicsError> {
    if columns == 0 {
        return Err(GraphicsError(format!(
            "Sheet needs at least one column, got {columns}"
        )));
    }
    if tiles.is_empty() {
        return Err(GraphicsError(
            "Cannot compose a sheet from zero tiles".into(),
        ));
    }
    let rows = tiles.len().div_ceil(columns);
    let width = columns * TILE_WIDTH;
    let height = rows * TILE_HEIGHT;
    let mut pixels = vec![0u8; width * height];
    for (index, tile) in tiles.iter().enumerate() {
        let ox = (index % columns) * TILE_WIDTH;
        let oy = (index / columns) * TILE_HEIGHT;
        for y in 0..TILE_HEIGHT {
            let start = (oy + y) * width + ox;
            pixels[start..start + TILE_WIDTH]
                .copy_from_slice(&tile[y * TILE_WIDTH..(y + 1) * TILE_WIDTH]);
        }
    }
    Ok((width, height, pixels))
}

/// `render_sheet`: an indexed PNG, the palette padded to a full line with
/// black; `transparent` is usually index 0, the sprites' transparent colour.
pub fn render_sheet(
    tiles: &[[u8; 64]],
    colors: &[[u8; 3]],
    columns: usize,
    transparent: Option<u8>,
) -> Result<Vec<u8>, GraphicsError> {
    let (width, height, pixels) = compose_sheet(tiles, columns)?;
    let mut palette = colors.to_vec();
    if palette.len() < COLORS_PER_LINE {
        palette.resize(COLORS_PER_LINE, [0, 0, 0]);
    }
    let transparent: Vec<u8> = transparent.into_iter().collect();
    Ok(png::encode_indexed(
        width as u32,
        height as u32,
        &pixels,
        &palette,
        &transparent,
    )?)
}

/// `_gap_bounds`: for each wanted offset, the distance to the next nonzero
/// landmark (or the image's end).
pub fn gap_bounds(
    landmarks: &[usize],
    rom_size: usize,
    wanted: &[usize],
) -> std::collections::BTreeMap<usize, usize> {
    let landmarks: std::collections::BTreeSet<usize> =
        landmarks.iter().copied().filter(|&o| o != 0).collect();
    wanted
        .iter()
        .copied()
        .filter(|&o| o != 0)
        .map(|offset| {
            let next = landmarks
                .range(offset + 1..)
                .next()
                .copied()
                .unwrap_or(rom_size);
            (offset, next - offset)
        })
        .collect()
}

/// What `decompress_art` checked and reports about one Nemesis blob.
pub struct ArtRecord {
    pub data: Vec<u8>,
    /// What the decoder read, which may be one more than the stream.
    pub consumed: usize,
    pub xor_mode: bool,
    /// The header word as stored.
    pub header_word: u16,
    /// The documented size when one was given, else what the decoder read.
    pub compressed_size: usize,
    pub tile_count: u16,
    pub compressed_sha256: String,
    pub decompressed_sha256: String,
}

/// `decompress_art`: one Nemesis blob, held to its header's pattern count and
/// to an exact documented size or an upper bound. `NemDecomp` may read one
/// lookahead byte it never uses, so either allows one byte more.
pub fn decompress_art(
    rom: &[u8],
    offset: usize,
    label: &str,
    exact: Option<usize>,
    bound: Option<usize>,
) -> Result<ArtRecord, crate::PackError> {
    use sha2::{Digest, Sha256};
    let refuse = |message: String| Err(crate::PackError(message));
    let header = crate::decode::nemesis::read_header(rom, offset)?;
    let (data, consumed) = crate::decode::nemesis::decompress(rom, offset)?;
    let declared = usize::from(header.row_count) * 4;
    if data.len() != declared {
        return refuse(format!(
            "{label} at 0x{offset:06X}: decoded {} bytes but the header declares {} patterns \
             ({declared} bytes)",
            data.len(),
            header.tile_count
        ));
    }
    if let Some(size) = exact
        && consumed != size
        && consumed != size + 1
    {
        return refuse(format!(
            "{label} at 0x{offset:06X}: decompressor consumed {consumed} bytes, but the \
             documented stream is {size} bytes (a trailing lookahead byte would allow at most \
             one more)"
        ));
    }
    if let Some(bound) = bound
        && consumed > bound + 1
    {
        return refuse(format!(
            "{label} at 0x{offset:06X}: decompressor consumed {consumed} bytes and ran into the \
             next blob, which starts {bound} bytes in"
        ));
    }
    let size = exact.unwrap_or(consumed);
    let compressed = rom.get(offset..offset + size).ok_or_else(|| {
        crate::PackError(format!(
            "{label} at 0x{offset:06X} (+{size}) runs past the end of the ROM"
        ))
    })?;
    Ok(ArtRecord {
        consumed,
        xor_mode: header.xor_mode,
        header_word: u16::from_be_bytes([rom[offset], rom[offset + 1]]),
        compressed_size: size,
        tile_count: header.tile_count,
        compressed_sha256: crate::png::hex(&Sha256::digest(compressed)),
        decompressed_sha256: crate::png::hex(&Sha256::digest(&data)),
        data,
    })
}

/// `planes.compose`: `words` drawn row-major, `columns` across, each cell's
/// pattern (counted from `art_vram_tile`) flipped as its word says and moved
/// onto its CRAM line; pattern index 0 stays the backdrop. Returns the width,
/// the height and the CRAM indices.
pub fn compose(
    words: &[u16],
    columns: usize,
    tiles: &[[u8; 64]],
    art_vram_tile: usize,
) -> Result<(usize, usize, Vec<u8>), crate::PackError> {
    if columns == 0 {
        return Err(crate::PackError(format!(
            "A plane needs at least one column, got {columns}"
        )));
    }
    if words.is_empty() {
        return Err(crate::PackError(
            "Cannot compose a plane from zero cells".into(),
        ));
    }
    if !words.len().is_multiple_of(columns) {
        return Err(crate::PackError(format!(
            "{} cells is not a whole number of {columns}-cell rows",
            words.len()
        )));
    }
    let (width, height) = (columns * 8, words.len() / columns * 8);
    let mut pixels = vec![0u8; width * height];
    for (index, &word) in words.iter().enumerate() {
        let tile = usize::from(word & 0x07FF);
        let art = tile
            .checked_sub(art_vram_tile)
            .and_then(|i| tiles.get(i))
            .ok_or_else(|| {
                crate::PackError(format!(
                    "Cell {index} references VRAM tile 0x{tile:03X}, which is art index {} of a \
                     {}-pattern blob loaded at VRAM tile 0x{art_vram_tile:03X}",
                    tile as i64 - art_vram_tile as i64,
                    tiles.len()
                ))
            })?;
        let (h_flip, v_flip) = (word & 0x0800 != 0, word & 0x1000 != 0);
        let base = ((word >> 13) & 3) as u8 * 16;
        let (ox, oy) = ((index % columns) * 8, (index / columns) * 8);
        for y in 0..8 {
            for x in 0..8 {
                let sx = if h_flip { 7 - x } else { x };
                let sy = if v_flip { 7 - y } else { y };
                let p = art[sy * 8 + sx];
                pixels[(oy + y) * width + ox + x] = if p == 0 { 0 } else { base + p };
            }
        }
    }
    Ok((width, height, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_decode_through_the_ramp() {
        let white = Color::decode(0x0EEE);
        assert_eq!(white.levels, [7, 7, 7]);
        assert_eq!(white.rgb(), [238, 238, 238]);
        let green = Color::decode(0x0040);
        assert_eq!(green.levels, [0, 2, 0]);
        assert_eq!(green.rgb(), [0, 68, 0]);
        assert_eq!(expand_channel(2, 'g'), Ok(68));
        assert!(expand_channel(8, 'r').is_err());
        assert!(expand_channel(1, 'x').is_err());
        let text = crate::json::dumps(&Color::decode(0x1001).to_json(), true);
        assert!(text.contains("\"unused_bits_set\": true"), "{text}");
        assert!(!crate::json::dumps(&white.to_json(), true).contains("unused"));
        assert!(decode_palette(&[0, 1, 2]).is_err());
        assert_eq!(GRAYSCALE_RAMP[1], [17, 17, 17]);
        assert_eq!(GRAYSCALE_RAMP[15], [255, 255, 255]);
    }

    #[test]
    fn tiles_and_sheets() {
        let mut chunk = [0u8; 32];
        chunk[0] = 0x12;
        let tile = decode_tile(&chunk).unwrap();
        assert_eq!(&tile[..3], &[1, 2, 0]);
        assert!(decode_tile(&chunk[..31]).is_err());
        assert!(decode_tiles(&[0; 33]).is_err());
        let (w, h, pixels) = compose_sheet(&[tile, [3; 64], [4; 64]], 2).unwrap();
        assert_eq!((w, h), (16, 16));
        assert_eq!(pixels[0], 1);
        assert_eq!(pixels[8], 3);
        assert_eq!(pixels[16 * 8], 4);
        assert_eq!(pixels[16 * 8 + 8], 0, "the unfilled cell stays index 0");
        assert!(compose_sheet(&[], 2).is_err());
        assert!(compose_sheet(&[tile], 0).is_err());
        assert!(render_sheet(&[tile], &GRAYSCALE_RAMP, 16, Some(0)).is_ok());
    }
}
