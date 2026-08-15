//! Battle presentation art and the data-only selectors that choose it.
//!
//! The PNGs in `runtime-pack/battle/art` are indexed renders.  Backgrounds
//! and character poses already carry the one palette they use.  Enemy bodies
//! deliberately do not: their pixels are CRAM indices and the battle slot's
//! position byte chooses CRAM line 1 or 2.  This module keeps that assembly
//! explicit instead of hiding it in a texture tint.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use godot::classes::{Image, ImageTexture, image::Format};
use godot::prelude::*;
use serde::Deserialize;

/// Parsed battle art, with no Godot objects retained between battles.
pub(crate) struct BattleArt {
    enemies: BTreeMap<u16, EnemyArt>,
    characters: BTreeMap<u8, CharacterArt>,
    backgrounds: BTreeMap<u8, String>,
    background_tables: BackgroundTables,
    enemy_palette: EnemyPaletteLayout,
}

#[derive(Clone)]
pub(crate) struct EnemyArt {
    pub(crate) width_cells: u16,
    pub(crate) height_cells: u16,
    pub(crate) png: String,
    pub(crate) palette_words: Vec<u16>,
}

#[derive(Clone)]
pub(crate) struct CharacterArt {
    pub(crate) poses: BTreeMap<u8, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BackgroundTables {
    pub(crate) event_battle: Vec<u8>,
    pub(crate) field_map: Vec<u8>,
    pub(crate) motavia_terrain: Vec<u8>,
    pub(crate) field_none: u8,
    pub(crate) dark_force_from: u8,
    pub(crate) dark_force_to: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EnemyPaletteLayout {
    pub(crate) first_table_index: usize,
    pub(crate) fixed_colors: BTreeMap<usize, u16>,
    pub(crate) ui_color_14: u16,
    pub(crate) index_15_by_line: BTreeMap<u8, u16>,
}

#[derive(Deserialize)]
struct EnemyFile {
    enemies: Vec<EnemyFileEntry>,
    palette_layout: EnemyPaletteFileLayout,
}

#[derive(Deserialize)]
struct EnemyFileEntry {
    id: u16,
    width_cells: u16,
    height_cells: u16,
    png: String,
    palette_words: Vec<String>,
}

#[derive(Deserialize)]
struct EnemyPaletteFileLayout {
    first_table_index: usize,
    fixed_colors: BTreeMap<String, String>,
    ui_color_14: String,
    index_15_by_cram_line: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct CharacterFile {
    characters: Vec<CharacterFileEntry>,
}

#[derive(Deserialize)]
struct CharacterFileEntry {
    id: u8,
    poses: Vec<CharacterPoseFileEntry>,
}

#[derive(Deserialize)]
struct CharacterPoseFileEntry {
    pose: u8,
    png: String,
}

#[derive(Deserialize)]
struct BackgroundFile {
    backgrounds: Vec<BackgroundFileEntry>,
    selection: BackgroundSelectionFile,
}

#[derive(Deserialize)]
struct BackgroundFileEntry {
    index: u8,
    png: String,
}

#[derive(Deserialize)]
struct BackgroundSelectionFile {
    event_battle: IndexTableFile,
    field_map: IndexTableFile,
    motavia_terrain: IndexTableFile,
    dark_force_2_swap: DarkForceSwapFile,
}

#[derive(Deserialize)]
struct IndexTableFile {
    indexes: Vec<u8>,
    #[serde(default = "default_no_background")]
    none: u8,
}

#[derive(Deserialize)]
struct DarkForceSwapFile {
    from: u8,
    to: u8,
}

fn default_no_background() -> u8 {
    u8::MAX
}

fn read_json<T: for<'de> Deserialize<'de>>(pack_dir: &str, name: &str) -> Result<T, String> {
    let path = Path::new(pack_dir).join(name);
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn parse_word(text: &str) -> Result<u16, String> {
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    u16::from_str_radix(digits, 16).map_err(|e| format!("invalid CRAM word {text:?}: {e}"))
}

impl BattleArt {
    pub(crate) fn load(pack_dir: &str) -> Result<BattleArt, String> {
        let enemy_file: EnemyFile = read_json(pack_dir, "battle/art/enemies.json")?;
        let character_file: CharacterFile = read_json(pack_dir, "battle/art/characters.json")?;
        let background_file: BackgroundFile = read_json(pack_dir, "battle/art/backgrounds.json")?;

        let enemies = enemy_file
            .enemies
            .into_iter()
            .map(|entry| {
                let palette_words = entry
                    .palette_words
                    .iter()
                    .map(String::as_str)
                    .map(parse_word)
                    .collect::<Result<Vec<_>, _>>()?;
                if palette_words.len() != 11 {
                    return Err(format!(
                        "enemy {} has {} palette words, expected 11",
                        entry.id,
                        palette_words.len()
                    ));
                }
                Ok((
                    entry.id,
                    EnemyArt {
                        width_cells: entry.width_cells,
                        height_cells: entry.height_cells,
                        png: entry.png,
                        palette_words,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;

        let characters = character_file
            .characters
            .into_iter()
            .map(|entry| {
                let poses = entry
                    .poses
                    .into_iter()
                    .map(|pose| (pose.pose, pose.png))
                    .collect();
                (entry.id, CharacterArt { poses })
            })
            .collect();

        let EnemyPaletteFileLayout {
            first_table_index,
            fixed_colors: fixed_color_words,
            ui_color_14,
            index_15_by_cram_line: index_15_words,
        } = enemy_file.palette_layout;
        let fixed_colors = fixed_color_words
            .iter()
            .map(|(index, word)| {
                let index = index
                    .parse::<usize>()
                    .map_err(|e| format!("invalid enemy palette index {index:?}: {e}"))?;
                Ok((index, parse_word(word)?))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let index_15_by_line = index_15_words
            .iter()
            .map(|(line, word)| {
                let line = line
                    .parse::<u8>()
                    .map_err(|e| format!("invalid enemy CRAM line {line:?}: {e}"))?;
                Ok((line, parse_word(word)?))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;

        Ok(BattleArt {
            enemies,
            characters,
            backgrounds: background_file
                .backgrounds
                .into_iter()
                .map(|entry| (entry.index, entry.png))
                .collect(),
            background_tables: BackgroundTables {
                event_battle: background_file.selection.event_battle.indexes,
                field_map: background_file.selection.field_map.indexes,
                motavia_terrain: background_file.selection.motavia_terrain.indexes,
                field_none: background_file.selection.field_map.none,
                dark_force_from: background_file.selection.dark_force_2_swap.from,
                dark_force_to: background_file.selection.dark_force_2_swap.to,
            },
            enemy_palette: EnemyPaletteLayout {
                first_table_index,
                fixed_colors,
                ui_color_14: parse_word(&ui_color_14)?,
                index_15_by_line,
            },
        })
    }

    pub(crate) fn background_path(
        &self,
        event_battle: Option<u16>,
        map_id: u16,
        motavia_terrain: Option<u8>,
        dark_force_2: bool,
    ) -> Option<&str> {
        let index = select_background(
            &self.background_tables,
            event_battle,
            map_id,
            motavia_terrain,
            dark_force_2,
        )?;
        self.backgrounds.get(&index).map(String::as_str)
    }

    pub(crate) fn background_path_by_index(&self, index: u8) -> Option<&str> {
        self.backgrounds.get(&index).map(String::as_str)
    }
    pub(crate) fn enemy_size(&self, enemy_id: u16) -> Option<(u16, u16)> {
        self.enemies
            .get(&enemy_id)
            .map(|art| (art.width_cells, art.height_cells))
    }

    pub(crate) fn enemy_texture(
        &self,
        pack_dir: &str,
        enemy_id: u16,
        position: u8,
    ) -> Option<Gd<ImageTexture>> {
        let art = self.enemies.get(&enemy_id)?;
        let line = enemy_cram_line(position);
        let palette = assemble_enemy_palette(&art.palette_words, line, &self.enemy_palette)?;
        let source_palette = assemble_enemy_palette(&art.palette_words, 1, &self.enemy_palette)?;
        let path = format!("{pack_dir}/{}", art.png);
        let mut image = Image::load_from_file(&GString::from(path.as_str()))?;
        image.convert(Format::RGBA8);

        let mut unresolved = false;
        for y in 0..image.get_height() {
            for x in 0..image.get_width() {
                let pixel = image.get_pixel(x, y);
                if pixel.a8() == 0 {
                    continue;
                }
                let Some(index) = palette_index(pixel, &source_palette) else {
                    unresolved = true;
                    continue;
                };
                let color = cram_color(palette[index]);
                image.set_pixel(x, y, color.with_alpha(f32::from(pixel.a8()) / 255.0));
            }
        }
        if unresolved {
            godot_error!(
                "battle art: {} contains a pixel outside its declared CRAM palette",
                art.png
            );
        }
        ImageTexture::create_from_image(&image)
    }

    pub(crate) fn character_texture(
        &self,
        pack_dir: &str,
        character_id: u8,
        pose: u8,
    ) -> Option<Gd<ImageTexture>> {
        // Runtime character ids are zero-based; the art extractor's ids are
        // one-based because they index the retail fighter blocks.
        let art = self.characters.get(&character_id.checked_add(1)?)?;
        let path = format!("{pack_dir}/{}", art.poses.get(&pose)?);
        let image = Image::load_from_file(&GString::from(path.as_str()))?;
        ImageTexture::create_from_image(&image)
    }
}

/// Selects the retail background number. `motavia_terrain` is the raw terrain
/// byte used to index `MotaBattleBGIndexes`; that table stores index+1 and
/// reserves zero for background zero.
pub(crate) fn select_background(
    tables: &BackgroundTables,
    event_battle: Option<u16>,
    map_id: u16,
    motavia_terrain: Option<u8>,
    dark_force_2: bool,
) -> Option<u8> {
    let selected = if let Some(event) = event_battle {
        tables.event_battle.get(usize::from(event)).copied()?
    } else if map_id == 0 {
        let terrain = usize::from(motavia_terrain?);
        tables
            .motavia_terrain
            .get(terrain)
            .copied()?
            .saturating_sub(1)
    } else {
        let index = tables.field_map.get(usize::from(map_id)).copied()?;
        if index == tables.field_none {
            return None;
        }
        index
    };
    Some(if dark_force_2 && selected == tables.dark_force_from {
        tables.dark_force_to
    } else {
        selected
    })
}

/// Bit 7 set selects the `$40` palette-table offset, CRAM line 2. Clear is
/// the `$20` offset, CRAM line 1 (`Palette_Table_Buffer` is line 0 at `$00`).
pub(crate) const fn enemy_cram_line(position: u8) -> u8 {
    if position & 0x80 != 0 { 2 } else { 1 }
}

/// Assembles the sixteen CRAM words the enemy body sees for one slot.
pub(crate) fn assemble_enemy_palette(
    palette_words: &[u16],
    line: u8,
    layout: &EnemyPaletteLayout,
) -> Option<[u16; 16]> {
    if !matches!(line, 1 | 2) || palette_words.len() != 11 {
        return None;
    }
    let mut colors = [0u16; 16];
    for (&index, &word) in &layout.fixed_colors {
        *colors.get_mut(index)? = word;
    }
    for (offset, word) in palette_words.iter().copied().enumerate() {
        *colors.get_mut(layout.first_table_index + offset)? = word;
    }
    colors[14] = layout.ui_color_14;
    colors[15] = *layout.index_15_by_line.get(&line)?;
    Some(colors)
}

fn palette_index(pixel: Color, palette: &[u16; 16]) -> Option<usize> {
    let rgb = (pixel.r8(), pixel.g8(), pixel.b8());
    (1..16).find(|&index| {
        let color = cram_color(palette[index]);
        (color.r8(), color.g8(), color.b8()) == rgb
    })
}

fn cram_color(word: u16) -> Color {
    Color::from_rgba8(
        expand_channel((word >> 1) & 7),
        expand_channel((word >> 5) & 7),
        expand_channel((word >> 9) & 7),
        255,
    )
}

fn expand_channel(level: u16) -> u8 {
    ((level << 5) | (level << 2) | (level >> 1)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_data::BattleFiles;

    fn tables() -> BackgroundTables {
        BackgroundTables {
            event_battle: vec![13, 4],
            field_map: vec![u8::MAX, 4, 21],
            motavia_terrain: vec![0, 1, 4],
            field_none: u8::MAX,
            dark_force_from: 4,
            dark_force_to: 5,
        }
    }

    #[test]
    fn background_selection_uses_event_map_and_terrain_tables() {
        let tables = tables();
        assert_eq!(
            select_background(&tables, Some(0), 2, None, false),
            Some(13)
        );
        assert_eq!(select_background(&tables, None, 1, None, false), Some(4));
        assert_eq!(select_background(&tables, None, 0, Some(2), false), Some(3));
        assert_eq!(select_background(&tables, None, 0, Some(1), false), Some(0));
        assert_eq!(select_background(&tables, None, 2, None, false), Some(21));
        assert_eq!(select_background(&tables, None, 2, None, true), Some(21));
    }

    #[test]
    fn dark_force_swap_applies_after_selection() {
        let tables = tables();
        assert_eq!(select_background(&tables, Some(1), 0, None, true), Some(5));
        assert_eq!(select_background(&tables, Some(1), 0, None, false), Some(4));
        assert_eq!(select_background(&tables, None, 99, None, false), None);
    }

    #[test]
    fn enemy_palette_places_fixed_table_and_line_dependent_words() {
        let layout = EnemyPaletteLayout {
            first_table_index: 3,
            fixed_colors: BTreeMap::from([(1, 0x0EEE), (2, 0)]),
            ui_color_14: 0x0600,
            index_15_by_line: BTreeMap::from([(1, 0x0CC4), (2, 0x062E)]),
        };
        let words: Vec<u16> = (0..11).map(|word| word + 0x100).collect();
        let line = assemble_enemy_palette(&words, 2, &layout).expect("valid palette");
        assert_eq!(line[0], 0);
        assert_eq!(line[1], 0x0EEE);
        assert_eq!(line[2], 0);
        assert_eq!(&line[3..14], words.as_slice());
        assert_eq!(line[14], 0x0600);
        assert_eq!(line[15], 0x062E);
    }

    #[test]
    fn enemy_position_bit_selects_the_two_retail_cram_lines() {
        assert_eq!(enemy_cram_line(14), 1);
        assert_eq!(enemy_cram_line(0x80 | 14), 2);
    }

    #[test]
    fn retail_battle_art_pack_has_the_declared_shape() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        let art = BattleArt::load(&pack.to_string_lossy()).expect("retail battle art loads");
        assert_eq!(art.enemies.len(), 153);
        assert_eq!(
            art.characters
                .values()
                .map(|character| character.poses.len())
                .sum::<usize>(),
            43
        );
        assert_eq!(art.backgrounds.len(), 32);
        assert_eq!(art.background_tables.event_battle.len(), 27);
        assert_eq!(art.background_tables.field_map.len(), 416);
        assert_eq!(art.background_tables.motavia_terrain.len(), 42);
    }

    #[test]
    fn igglanova_boss_uses_pack_positions_palette_and_event_background() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        let files = BattleFiles::load(&pack).expect("retail battle pack loads");
        let formation = files
            .formations
            .boss_formations
            .iter()
            .find(|formation| formation.event_battle_index == Some(0))
            .expect("Igglanova event battle exists");

        assert_eq!(formation.run_chance, 0xFE);
        assert_eq!(
            formation
                .enemies
                .iter()
                .map(|enemy| (enemy.enemy_id, enemy.position))
                .collect::<Vec<_>>(),
            vec![(9, 137), (12, 20), (9, 159)]
        );

        let art = BattleArt::load(&pack.to_string_lossy()).expect("retail battle art loads");
        for enemy_id in [9, 12] {
            let entry = art.enemies.get(&enemy_id).expect("boss enemy art exists");
            assert_eq!(entry.palette_words.len(), 11);
            assert!(!entry.png.is_empty());
        }
        assert_eq!(enemy_cram_line(20), 1);
        assert_eq!(enemy_cram_line(137), 2);
        assert_eq!(enemy_cram_line(159), 2);
        assert_eq!(
            art.background_path(Some(0), 0x17, None, false),
            Some("battle/art/backgrounds/13_AcademyBasement.png")
        );
    }
}
