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

/// `GPGX_RGB565_RAMP`: each channel's eight levels.
pub const RAMP_R: [u8; 8] = [0, 32, 65, 98, 139, 172, 205, 238];
pub const RAMP_G: [u8; 8] = [0, 32, 68, 101, 137, 170, 206, 238];
pub const RAMP_B: [u8; 8] = [0, 32, 65, 98, 139, 172, 205, 238];

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
