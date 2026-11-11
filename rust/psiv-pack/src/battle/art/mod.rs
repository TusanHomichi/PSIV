//! `battle/art/`: the port of `psiv_tools/battle_art_pack.py` and the
//! modules it calls, which own the reasons for each file (#216, slice 6e).
//! This part writes the command-window and damage-number tiles, the Fusion
//! frames, the enemy bodies, their overlays and attack sheets, the character
//! poses and the backgrounds.

mod attacks;
mod backgrounds;
mod characters;
pub(super) mod enemies;
mod fusion;
mod overlays;
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

pub use crate::gfx::compose;

/// Write a file after reading it back as the type the game loads.
fn write_checked<T: serde::de::DeserializeOwned>(
    root: &Path,
    relative: &str,
    value: &Json,
) -> Result<String, PackError> {
    let mut checked = value.clone();
    checked.insert("format_version", super::FORMAT_VERSION);
    serde_json::from_value::<T>(serde_json::Value::from(&checked))
        .map_err(|error| PackError(format!("{relative} does not read back: {error}")))?;
    crate::write_json(root, relative, &checked)
}

/// `ENEMY_CRAM_LINES` (`psiv_tools/battle_art.py`): the lines an enemy body
/// draws with.
const ENEMY_CRAM_LINES: [u8; 2] = [1, 2];

/// One `files` entry: the index, its digest and count, and its PNGs.
fn file_entry(
    root: &Path,
    name: &str,
    sha: String,
    count: Json,
    pngs: Option<&str>,
) -> Result<Json, PackError> {
    let entry = Json::object()
        .with("file", name)
        .with("sha256", sha)
        .with("count", count);
    Ok(match pngs {
        Some(directory) => {
            let (files, bytes) = crate::png_census(root, directory)?;
            entry
                .with("png_directory", directory)
                .with("png_count", files)
                .with("png_bytes", bytes)
        }
        None => entry.with("png_count", 0u8),
    })
}

/// `battle/art/command_ui.json`, `battle/art/damage_digits.json`,
/// `battle/art/fusion.json`, `battle/art/enemies.json`,
/// `battle/art/characters.json`, `battle/art/backgrounds.json`,
/// `battle/art/enemy_overlays.json` and `battle/art/enemy_attacks.json`
/// (drawn from `enemy_animations`, the file this pack writes beside it), each
/// index with its PNGs.
pub fn build(rom: &[u8], root: &Path, enemy_animations: &Json) -> Result<Json, PackError> {
    let command_ui = tiles::command_ui(rom)?;
    let command_ui_sha =
        write_checked::<psiv_data::CommandUiArt>(root, "battle/art/command_ui.json", &command_ui)?;
    let digits = tiles::damage_digits(rom)?;
    let digits_sha =
        write_checked::<psiv_data::DamageDigitArt>(root, "battle/art/damage_digits.json", &digits)?;
    let fusion = fusion::write(rom, root)?;
    let fusion_sha =
        write_checked::<psiv_data::FusionArt>(root, "battle/art/fusion.json", &fusion)?;
    let enemies = enemies::write(rom, root)?;
    let enemies_sha = crate::write_json(root, "battle/art/enemies.json", &enemies)?;
    let characters = characters::write(rom, root)?;
    let characters_sha = crate::write_json(root, "battle/art/characters.json", &characters)?;
    let backgrounds = backgrounds::write(rom, root)?;
    let backgrounds_sha = crate::write_json(root, "battle/art/backgrounds.json", &backgrounds)?;
    let overlays = overlays::write(rom, root)?;
    let overlays_sha = crate::write_json(root, "battle/art/enemy_overlays.json", &overlays)?;
    let attacks = attacks::write(rom, root, enemy_animations)?;
    let attacks_sha = crate::write_json(root, "battle/art/enemy_attacks.json", &attacks)?;

    let files = Json::object()
        .with(
            "enemies",
            file_entry(
                root,
                "battle/art/enemies.json",
                enemies_sha,
                enemies.at(&["count"])?.clone(),
                Some("battle/art/enemies"),
            )?,
        )
        .with(
            "characters",
            file_entry(
                root,
                "battle/art/characters.json",
                characters_sha,
                characters.at(&["count"])?.clone(),
                Some("battle/art/characters"),
            )?,
        )
        .with(
            "backgrounds",
            file_entry(
                root,
                "battle/art/backgrounds.json",
                backgrounds_sha,
                backgrounds.at(&["count"])?.clone(),
                Some("battle/art/backgrounds"),
            )?,
        )
        .with(
            "enemy_overlays",
            file_entry(
                root,
                "battle/art/enemy_overlays.json",
                overlays_sha,
                overlays.at(&["piece_count"])?.clone(),
                Some("battle/art/enemy_overlays"),
            )?,
        )
        .with(
            "command_ui",
            file_entry(
                root,
                "battle/art/command_ui.json",
                command_ui_sha,
                command_ui.at(&["tile_count"])?.clone(),
                None,
            )?,
        )
        .with(
            "damage_digits",
            file_entry(
                root,
                "battle/art/damage_digits.json",
                digits_sha,
                digits.at(&["tile_count"])?.clone(),
                None,
            )?,
        )
        .with(
            "fusion",
            file_entry(
                root,
                "battle/art/fusion.json",
                fusion_sha,
                fusion.at(&["count"])?.clone(),
                Some("battle/art/fusion"),
            )?,
        )
        .with(
            "enemy_attacks",
            file_entry(
                root,
                "battle/art/enemy_attacks.json",
                attacks_sha,
                attacks.at(&["count"])?.clone(),
                Some("battle/art/enemy_attacks"),
            )?,
        );
    Ok(Json::object()
        .with("directory", "battle/art")
        .with("files", files)
        .with(
            "census",
            census(
                &enemies,
                &overlays,
                &attacks,
                &characters,
                &backgrounds,
                &command_ui,
                &digits,
            )?,
        )
        .with(
            "note",
            "Body layer plus the additive animated sprite-piece overlay. The overlay census \
             records the 87 static hole cells whose decoded destinations do not fully cover a \
             body.",
        ))
}

/// `emit_battle_art`'s census, read back from the files it describes.
fn census(
    enemies: &Json,
    overlays: &Json,
    attacks: &Json,
    characters: &Json,
    backgrounds: &Json,
    command_ui: &Json,
    digits: &Json,
) -> Result<Json, PackError> {
    let bodies = enemies.at(&["enemies"])?.items()?;
    let complete = bodies
        .iter()
        .filter(|e| e.get("body_complete") == Some(&Json::Bool(true)))
        .count();
    let mut holes = 0;
    let mut formats: std::collections::BTreeMap<String, usize> = Default::default();
    for body in bodies {
        holes += body.at(&["body_holes"])?.int()?;
        let Json::Str(kind) = body.at(&["mapping", "format"])? else {
            return Err(PackError("an enemy mapping format is not text".into()));
        };
        *formats.entry(kind.clone()).or_default() += 1;
    }
    let poses = characters.at(&["characters"])?.items()?;
    let mut per_character = Json::object();
    let mut blobs = 0;
    for entry in poses {
        let Json::Str(symbol) = entry.at(&["symbol"])? else {
            return Err(PackError("a character symbol is not text".into()));
        };
        per_character.insert(symbol.clone(), entry.at(&["pose_count"])?.clone());
        blobs += entry.at(&["art"])?.items()?.len();
    }
    let listed = backgrounds.at(&["backgrounds"])?.items()?;
    let never: Vec<Json> = listed
        .iter()
        .filter(|e| e.get("selectable") != Some(&Json::Bool(true)))
        .map(|e| e.at(&["index"]).cloned())
        .collect::<Result<_, _>>()?;
    let field = backgrounds
        .at(&["selection", "field_map", "indexes"])?
        .items()?;
    let none = field
        .iter()
        .filter(|v| **v == Json::from(backgrounds::NO_BACKGROUND))
        .count();
    Ok(Json::object()
        .with("enemies", enemies.at(&["count"])?.clone())
        .with("body_complete", complete)
        .with("body_holes", bodies.len() - complete)
        .with("total_body_holes", holes)
        .with(
            "enemy_overlay_pieces",
            overlays.at(&["piece_count"])?.clone(),
        )
        .with(
            "enemy_overlay_enabled_pieces",
            overlays.at(&["enabled_piece_count"])?.clone(),
        )
        .with(
            "enemy_overlay_coverage",
            overlays.at(&["coverage"])?.clone(),
        )
        .with("enemy_attack_sheets", attacks.at(&["census"])?.clone())
        .with(
            "enemy_mapping_formats",
            Json::Object(
                formats
                    .into_iter()
                    .map(|(kind, count)| (kind, Json::from(count)))
                    .collect(),
            ),
        )
        .with("enemy_cram_lines", ENEMY_CRAM_LINES.to_vec())
        .with(
            "enemy_body_color_indices",
            enemies
                .at(&["palette_layout", "body_color_indices_used"])?
                .clone(),
        )
        .with("command_ui_tiles", command_ui.at(&["tile_count"])?.clone())
        .with("damage_digit_tiles", digits.at(&["tile_count"])?.clone())
        .with("characters", characters.at(&["count"])?.clone())
        .with("poses", characters.at(&["pose_count"])?.clone())
        .with("poses_per_character", per_character)
        .with(
            "character_palette_variants",
            characters.at(&["palette", "table", "entry_count"])?.clone(),
        )
        .with("character_art_blobs", blobs)
        .with("backgrounds", backgrounds.at(&["count"])?.clone())
        .with(
            "background_art_blobs",
            backgrounds.at(&["distinct_art_blobs"])?.clone(),
        )
        .with("backgrounds_selectable", listed.len() - never.len())
        .with("backgrounds_never_selectable", Json::Array(never))
        .with("background_maps_bound", field.len() - none)
        .with("background_maps_none", none)
        .with(
            "background_event_battles",
            backgrounds
                .at(&["selection", "event_battle", "indexes"])?
                .items()?
                .len(),
        ))
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
