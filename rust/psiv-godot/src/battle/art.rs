//! Battle presentation art. Which background a battle draws is the runtime's
//! (`psiv_runtime::Runtime::battle_background`); this module only loads it.
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

use super::enemy_overlay::{
    AnimationPiece, AttackAnimation, EnemyAnimation, EnemyAttackArtFile, EnemyAttackArtFileEntry,
    EnemyOverlayFile, EnemyOverlayFileEntry,
};

/// Parsed battle art, with no Godot objects retained between battles.
pub(crate) struct BattleArt {
    enemies: BTreeMap<u16, EnemyArt>,
    characters: BTreeMap<u8, CharacterArt>,
    backgrounds: BTreeMap<u8, String>,
    enemy_overlays: BTreeMap<u16, EnemyOverlayFileEntry>,
    enemy_attacks: BTreeMap<u16, EnemyAttackArtFileEntry>,
    enemy_palette: EnemyPaletteLayout,
    /// The Zol slugs' Fusion frames, which `psiv_data::FusionArt` owns.
    fusion: psiv_data::FusionArt,
}

/// One Fusion frame as the stage draws it: the texture, its top-left
/// relative to the piece's position, and its size.
pub(crate) struct FusionFrame {
    pub(crate) texture: Gd<ImageTexture>,
    pub(crate) origin: Vector2i,
    pub(crate) size: Vector2i,
}

#[derive(Clone)]
pub(crate) struct EnemyArt {
    /// The ROM position byte is the right-edge/half-width anchor, not the
    /// rendered body's full width. The generated art record supplies this
    /// anchor alongside the full-width metadata used during pack validation.
    pub(crate) half_width_cells: u16,
    pub(crate) height_cells: u16,
    pub(crate) png: String,
    pub(crate) palette_words: Vec<u16>,
}

#[derive(Clone)]
pub(crate) struct CharacterArt {
    pub(crate) poses: BTreeMap<u8, String>,
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
    half_width_cells: u16,
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
}

#[derive(Deserialize)]
struct BackgroundFileEntry {
    index: u8,
    png: String,
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
        let overlay_file: EnemyOverlayFile = read_json(pack_dir, "battle/art/enemy_overlays.json")?;
        let attack_file: EnemyAttackArtFile = read_json(pack_dir, "battle/art/enemy_attacks.json")?;
        let character_file: CharacterFile = read_json(pack_dir, "battle/art/characters.json")?;
        let background_file: BackgroundFile = read_json(pack_dir, "battle/art/backgrounds.json")?;
        let fusion = psiv_data::FusionArt::load(Path::new(pack_dir)).map_err(|e| e.to_string())?;

        let enemies = enemy_file
            .enemies
            .into_iter()
            .map(|entry| {
                if entry.width_cells != entry.half_width_cells.saturating_mul(2) {
                    return Err(format!(
                        "enemy {} width {} is not twice half-width {}",
                        entry.id, entry.width_cells, entry.half_width_cells
                    ));
                }
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
                        half_width_cells: entry.half_width_cells,
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

        let enemy_overlays = overlay_file
            .enemies
            .into_iter()
            .map(|entry| (entry.id, entry))
            .collect();
        let enemy_attacks = attack_file
            .enemies
            .into_iter()
            .map(|entry| (entry.id, entry))
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
            enemy_overlays,
            enemy_attacks,
            enemy_palette: EnemyPaletteLayout {
                first_table_index,
                fixed_colors,
                ui_color_14: parse_word(&ui_color_14)?,
                index_15_by_line,
            },
            fusion,
        })
    }

    /// The Fusion frames as textures, in the pack's order. The pieces' pattern
    /// word carries no line bits (`#$A000`, `ps4.asm:35726`), so they draw on
    /// CRAM line 1 in the casting slug's colours.
    pub(crate) fn fusion_frames(&self, pack_dir: &str) -> Option<Vec<FusionFrame>> {
        let art = self.enemies.get(&self.fusion.palette_enemy_id)?;
        self.fusion
            .frames
            .iter()
            .map(|frame| {
                let image = self.enemy_image(pack_dir, art, &frame.png, 0)?;
                Some(FusionFrame {
                    texture: ImageTexture::create_from_image(&image)?,
                    origin: Vector2i::new(frame.origin[0], frame.origin[1]),
                    size: Vector2i::new(frame.size[0], frame.size[1]),
                })
            })
            .collect()
    }

    /// CRAM index 15 of lines 0 to 3: the ink the menu font and the HP/TP
    /// labels take on each line. Lines 0 to 2 are the layout's per-line
    /// words (`ps4.asm:7654`, `loc_7634`); line 3 is the window palette's
    /// white (`loc_76A4`).
    pub(crate) fn line_ink(&self) -> [Color; 4] {
        let word = |line: u8| {
            self.enemy_palette
                .index_15_by_line
                .get(&line)
                .copied()
                .unwrap_or(0x0EEE)
        };
        [
            cram_color(word(0)),
            cram_color(word(1)),
            cram_color(word(2)),
            cram_color(0x0EEE),
        ]
    }

    pub(crate) fn background_path_by_index(&self, index: u8) -> Option<&str> {
        self.backgrounds.get(&index).map(String::as_str)
    }
    /// The enemy records with art.
    pub(crate) fn enemy_count(&self) -> usize {
        self.enemies.len()
    }

    pub(crate) fn enemy_size(&self, enemy_id: u16) -> Option<(u16, u16)> {
        self.enemies
            .get(&enemy_id)
            .map(|art| (art.half_width_cells, art.height_cells))
    }

    pub(crate) fn enemy_texture(
        &self,
        pack_dir: &str,
        enemy_id: u16,
        position: u8,
    ) -> Option<Gd<ImageTexture>> {
        let art = self.enemies.get(&enemy_id)?;
        let image = self.enemy_image(pack_dir, art, &art.png, position)?;
        ImageTexture::create_from_image(&image)
    }

    pub(crate) fn enemy_animation(
        &self,
        pack_dir: &str,
        enemy_id: u16,
        position: u8,
        initial_phase_ticks: usize,
    ) -> Option<EnemyAnimation> {
        let art = self.enemies.get(&enemy_id)?;
        let base = self.enemy_image(pack_dir, art, &art.png, position)?;
        let mut pieces = Vec::new();
        if let Some(overlay) = self.enemy_overlays.get(&enemy_id) {
            for piece in &overlay.pieces {
                if !piece.enabled || piece.placements.is_empty() {
                    continue;
                }
                let Some(initial_path) = piece.initial_png.as_ref() else {
                    godot_error!("battle enemy {enemy_id} overlay piece has no initial frame");
                    return None;
                };
                if piece.frames.len() != piece.durations.len() || piece.frames.is_empty() {
                    godot_error!(
                        "battle enemy {enemy_id} overlay piece has {} frames and {} durations",
                        piece.frames.len(),
                        piece.durations.len()
                    );
                    return None;
                }
                let initial = self.enemy_image(pack_dir, art, initial_path, position)?;
                let frames = piece
                    .frames
                    .iter()
                    .map(|frame| self.enemy_image(pack_dir, art, &frame.png, position))
                    .collect::<Option<Vec<_>>>()?;
                pieces.push(AnimationPiece {
                    initial,
                    frames,
                    durations: piece.durations.clone(),
                    placements: piece.placements.clone(),
                });
            }
        }
        let attack = self.enemy_attack_animation(pack_dir, art, enemy_id, position)?;
        EnemyAnimation::new(base, pieces, attack, initial_phase_ticks)
    }

    fn enemy_attack_animation(
        &self,
        pack_dir: &str,
        art: &EnemyArt,
        enemy_id: u16,
        position: u8,
    ) -> Option<Option<AttackAnimation>> {
        let Some(entry) = self.enemy_attacks.get(&enemy_id) else {
            return Some(None);
        };
        if entry.status != "exact" || entry.frames.is_empty() {
            return Some(None);
        }
        let Some(origin) = entry.origin_pixels else {
            godot_error!("battle enemy {enemy_id} attack art has no canvas origin");
            return None;
        };
        let mut textures = Vec::with_capacity(entry.frames.len());
        for (expected_index, frame) in entry.frames.iter().enumerate() {
            if frame.index != expected_index || frame.duration == 0 {
                godot_error!("battle enemy {enemy_id} attack frame index/timing is malformed");
                return None;
            }
            let image = self.enemy_image(pack_dir, art, &frame.png, position)?;
            textures.push(ImageTexture::create_from_image(&image)?);
        }
        let runtime = &entry.movement.runtime;
        let initial = if entry.movement.status == "exact" {
            runtime.initial_offset_pixels
        } else {
            [0, 0]
        };
        let _motion_kind = runtime.kind.as_str();
        Some(Some(AttackAnimation {
            frames: textures,
            origin_pixels: Vector2i::new(origin[0], origin[1]),
            initial_offset_pixels: Vector2i::new(initial[0], initial[1]),
            step_pixels: Vector2i::new(runtime.step_pixels[0], runtime.step_pixels[1]),
            limit_offset_pixels: runtime
                .limit_offset_pixels
                .map(|limit| Vector2i::new(limit[0], limit[1])),
        }))
    }

    fn enemy_image(
        &self,
        pack_dir: &str,
        art: &EnemyArt,
        relative_path: &str,
        position: u8,
    ) -> Option<Gd<Image>> {
        let line = enemy_cram_line(position);
        let palette = assemble_enemy_palette(&art.palette_words, line, &self.enemy_palette)?;
        let source_palette = assemble_enemy_palette(&art.palette_words, 1, &self.enemy_palette)?;
        let path = format!("{pack_dir}/{relative_path}");
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
                "battle art: {relative_path} contains a pixel outside its declared CRAM palette"
            );
        }
        Some(image)
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

pub(crate) fn cram_color(word: u16) -> Color {
    let [r, g, b] = psiv_data::color::cram_rgb(word);
    Color::from_rgba8(r, g, b, 255)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_data::BattleFiles;

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
    fn cram_color_uses_the_gpgx_rgb565_ramps() {
        assert_eq!(
            (
                cram_color(0x0AAA).r8(),
                cram_color(0x0AAA).g8(),
                cram_color(0x0AAA).b8()
            ),
            (172, 170, 172)
        );
        assert_eq!(
            (
                cram_color(0x0620).r8(),
                cram_color(0x0620).g8(),
                cram_color(0x0620).b8()
            ),
            (0, 32, 98)
        );
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
        let tables = psiv_data::BattleBackgrounds::load(&pack)
            .expect("retail background tables load")
            .expect("the retail pack has them");
        let index = tables.select(Some(0), 0x17, None, false, false);
        assert_eq!(
            index.and_then(|index| art.background_path_by_index(index)),
            Some("battle/art/backgrounds/13_AcademyBasement.png")
        );
    }
}
