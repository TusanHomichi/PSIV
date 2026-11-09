//! `battle/art/`: the port of `psiv_tools/battle_art_pack.py` and the
//! modules it calls, which own the reasons for each file (#216, slice 6e).
//! This part writes the command-window and damage-number tiles, the Fusion
//! frames, the enemy bodies and the character poses; the overlays, the attack
//! sheets and the backgrounds follow.

mod characters;
pub(super) mod enemies;
mod fusion;
mod tiles;

use std::path::Path;

use crate::json::Json;
use crate::{PackError, gfx};

/// `ENEMY_PALETTE_TABLE`: eleven colour words per enemy, 22 bytes apart.
pub const ENEMY_PALETTE_TABLE: usize = 0x27FFA2;
pub const ENEMY_PALETTE_STRIDE: usize = 22;
pub const ENEMY_PALETTE_COLORS: usize = 11;
pub const ENEMY_PALETTE_FIRST_INDEX: usize = 3;
/// `loc_10ED4`: `move.w #$EEE,(a1)+` / `clr.w (a1)+` before the copy.
pub const ENEMY_FIXED_COLORS: [(usize, u16); 2] = [(1, 0x0EEE), (2, 0x0000)];
/// `UI_COLOR_14`: written to lines 0, 1 and 2.
pub const UI_COLOR_14: u16 = 0x0600;
pub const TRANSPARENT_INDEX: usize = 0;
pub const LINE_DEPENDENT_INDEX: usize = 15;

/// `UI_COLOR_SITES`: the colours `loc_7634` writes as instruction
/// immediates (ROM offset, CRAM line, colour index, value), checked before
/// they are believed (`characters::verify_ui_colors`).
pub const UI_COLOR_SITES: [(usize, usize, usize, u16); 3] = [
    (0x007672, 0, 15, 0x02CE),
    (0x007678, 1, 15, 0x0CC4),
    (0x00767E, 2, 15, 0x062E),
];

/// `_safe`: a label as a file name.
pub fn safe(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// `enemy_palette_words`: the eleven words `loc_27FFA2` holds for an enemy.
pub fn enemy_palette_words(rom: &[u8], enemy_id: usize) -> Result<Vec<u16>, PackError> {
    let at = ENEMY_PALETTE_TABLE + enemy_id * ENEMY_PALETTE_STRIDE;
    let raw = rom.get(at..at + ENEMY_PALETTE_COLORS * 2).ok_or_else(|| {
        PackError(format!(
            "enemy palette 0x{enemy_id:02X} at 0x{at:06X} (+{}) runs past the end of the ROM",
            ENEMY_PALETTE_COLORS * 2
        ))
    })?;
    Ok(raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect())
}

/// `enemy_palette`: the sixteen colours an enemy's CRAM line holds, with the
/// transparent index and the line-dependent one left black.
pub fn enemy_palette(rom: &[u8], enemy_id: usize) -> Result<Vec<[u8; 3]>, PackError> {
    let mut words = [0u16; 16];
    for (index, value) in ENEMY_FIXED_COLORS {
        words[index] = value;
    }
    for (i, word) in enemy_palette_words(rom, enemy_id)?.into_iter().enumerate() {
        words[ENEMY_PALETTE_FIRST_INDEX + i] = word;
    }
    words[14] = UI_COLOR_14;
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
    let mut colors = gfx::palette_rgb(&gfx::decode_palette(&bytes)?);
    colors[TRANSPARENT_INDEX] = [0, 0, 0];
    colors[LINE_DEPENDENT_INDEX] = [0, 0, 0];
    Ok(colors)
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
) -> Result<(usize, usize, Vec<u8>), PackError> {
    if columns == 0 {
        return Err(PackError(format!(
            "A plane needs at least one column, got {columns}"
        )));
    }
    if words.is_empty() {
        return Err(PackError("Cannot compose a plane from zero cells".into()));
    }
    if !words.len().is_multiple_of(columns) {
        return Err(PackError(format!(
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
                PackError(format!(
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

/// Write a file after reading it back as the type the game loads.
fn write_checked<T: serde::de::DeserializeOwned>(
    root: &Path,
    relative: &str,
    value: &Json,
) -> Result<(), PackError> {
    let mut checked = value.clone();
    checked.insert("format_version", super::FORMAT_VERSION);
    serde_json::from_value::<T>(serde_json::Value::from(&checked))
        .map_err(|error| PackError(format!("{relative} does not read back: {error}")))?;
    crate::write_json(root, relative, &checked).map(drop)
}

/// `battle/art/command_ui.json`, `battle/art/damage_digits.json`,
/// `battle/art/fusion.json`, `battle/art/enemies.json` and
/// `battle/art/characters.json`, each index with its PNGs.
pub fn build(rom: &[u8], root: &Path) -> Result<(), PackError> {
    write_checked::<psiv_data::CommandUiArt>(
        root,
        "battle/art/command_ui.json",
        &tiles::command_ui(rom)?,
    )?;
    write_checked::<psiv_data::DamageDigitArt>(
        root,
        "battle/art/damage_digits.json",
        &tiles::damage_digits(rom)?,
    )?;
    write_checked::<psiv_data::FusionArt>(
        root,
        "battle/art/fusion.json",
        &fusion::write(rom, root)?,
    )?;
    crate::write_json(root, "battle/art/enemies.json", &enemies::write(rom, root)?)?;
    crate::write_json(
        root,
        "battle/art/characters.json",
        &characters::write(rom, root)?,
    )
    .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_enemy_palette_fixes_its_ui_colours_and_blanks_the_line_dependent_ones() {
        let mut rom = vec![0u8; ENEMY_PALETTE_TABLE + 2 * ENEMY_PALETTE_STRIDE];
        let at = ENEMY_PALETTE_TABLE + ENEMY_PALETTE_STRIDE;
        rom[at..at + 2].copy_from_slice(&0x000Eu16.to_be_bytes());
        let colors = enemy_palette(&rom, 1).unwrap();
        assert_eq!(colors.len(), 16);
        assert_eq!(colors[0], [0, 0, 0]);
        assert_eq!(colors[15], [0, 0, 0]);
        assert_eq!(colors[1], gfx::Color::decode(0x0EEE).rgb());
        assert_eq!(colors[3], gfx::Color::decode(0x000E).rgb());
        assert_eq!(colors[14], gfx::Color::decode(0x0600).rgb());
        assert!(enemy_palette(&rom, 2).is_err());
    }
}
