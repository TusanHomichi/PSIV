//! Retail front-door presentation: Sega logo, title art, and save menu.
//!
//! The title's flow is the runtime's: `psiv-runtime/src/session/title.rs` owns
//! the phases, the option rows, the slot rules, the erase confirmation and the
//! frames each phase runs for, and it hands this file a [`TitleView`] per
//! frame. What is left here is the picture — the five art placements, the two
//! menu windows, the CRAM replay and the prompt's palette cycle — sized from
//! the decoded oracle constants in `oracle/decode_layout.py`. The nodes remain
//! children of `Field`, and hiding them hands control back to the field shell
//! without changing scenes or save serialization.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use godot::classes::{ColorRect, Image, ImageTexture, Sprite2D};
use godot::obj::BaseMut;
use godot::prelude::*;
use psiv_data::{DialogueSet, Role};
use psiv_runtime::{TitlePhase, TitleView, TitleWindow};

use super::Field;
use crate::pack_assets::PackAssets;

const SCREEN_WIDTH: f32 = 320.0;
const SCREEN_HEIGHT: f32 = 224.0;
const CAMERA_ZOOM: f32 = 3.0;
const VIEW_WIDTH: f32 = 1280.0 / CAMERA_ZOOM;
const VIEW_HEIGHT: f32 = 800.0 / CAMERA_ZOOM;
const CELL: f32 = 8.0;

/// The reveal's art window: the decoded title mappings settle by frame 248 of
/// the transfer (oracle tape 25). Presentation only — the phase's own length is
/// the runtime's.
const TITLE_ART_APPEAR_TICKS: u32 = 248;

/// `Main_Frame_Count` is 93 frames behind the tape frame number when the
/// title routine starts in the Grand Cross build. The oracle receipts at
/// frames 500/550/600 therefore sample counts 407/457/507. The prompt's
/// palette table is indexed from that counter, not from the title phase.
const TITLE_MAIN_FRAME_OFFSET: u32 = 93;
const PRESS_START_PALETTE: [u16; 32] = [
    0x04EE, 0x04EE, 0x04EE, 0x04EE, 0x04EE, 0x04EE, 0x04EE, 0x04EE, 0x04CE, 0x04AE, 0x048E, 0x046E,
    0x044E, 0x042E, 0x040E, 0x020C, 0x000A, 0x000A, 0x000A, 0x000A, 0x000A, 0x000A, 0x000A, 0x000A,
    0x020C, 0x040E, 0x042E, 0x044E, 0x046E, 0x048E, 0x04AE, 0x04CE,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuKind {
    NoSave,
    SaveOptions,
    Slots,
    EraseConfirm,
    SoundTest,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    Black,
    SegaLogo,
    Background,
    BackgroundTransfer,
    TitleLogo,
    Subtitle,
    PressStart,
    Copyright,
    Menu {
        kind: MenuKind,
        option: Option<usize>,
    },
    SoundTestTrack {
        track: usize,
    },
}

enum VisualNode {
    Sprite(Gd<Sprite2D>),
    Rect(Gd<ColorRect>),
}

struct Visual {
    node: VisualNode,
    offset: Vector2,
    surface: Surface,
    replay: BTreeMap<u32, Gd<ImageTexture>>,
    press_cycle: Vec<Gd<ImageTexture>>,
}

#[derive(serde::Deserialize)]
struct TitleReplayManifest {
    frames: BTreeMap<String, serde_json::Value>,
}

/// Nodes for the front door, over the field's own.
///
/// The nodes remain children of `Field` so the Godot scene stays untouched;
/// hiding them hands control back to the field shell without changing scenes
/// or save serialization. The flow they draw is not here: the session's
/// [`TitleView`] says which window is up, where the cursor sits and how many
/// frames the phase has run.
pub(crate) struct TitleScreen {
    visuals: Vec<Visual>,
    replay_frames: Vec<u32>,
}

impl TitleScreen {
    pub(crate) fn build(
        assets: &PackAssets,
        dialogue: &DialogueSet,
        mut parent: BaseMut<'_, Field>,
    ) -> Option<TitleScreen> {
        let mut screen = TitleScreen {
            visuals: Vec::new(),
            replay_frames: load_replay_frames(assets),
        };

        screen.add_black(&mut parent);
        screen.add_image(
            &mut parent,
            assets,
            "title/background.png",
            Surface::Background,
            true,
            90,
        )?;
        screen.add_image(
            &mut parent,
            assets,
            "title/titlebarbgtoppart.png",
            Surface::BackgroundTransfer,
            false,
            90,
        )?;
        screen.add_image(
            &mut parent,
            assets,
            "title/sega_logo.png",
            Surface::SegaLogo,
            false,
            91,
        )?;
        screen.add_image(
            &mut parent,
            assets,
            "title/title_logo.png",
            Surface::TitleLogo,
            false,
            91,
        )?;
        screen.add_image(
            &mut parent,
            assets,
            "title/subtitle.png",
            Surface::Subtitle,
            false,
            91,
        )?;
        screen.add_image(
            &mut parent,
            assets,
            "title/press_start.png",
            Surface::PressStart,
            false,
            91,
        )?;
        screen.add_image(
            &mut parent,
            assets,
            "title/copyright.png",
            Surface::Copyright,
            false,
            91,
        )?;

        if !screen.add_menu_art(&mut parent, assets, dialogue) {
            screen.hide();
            return None;
        }
        screen.present(&TitleView {
            phase: TitlePhase::Sega,
            window: TitleWindow::None,
            cursor: 0,
            slots: [false; 3],
            ticks: 0,
            elapsed: 0,
        });
        Some(screen)
    }

    fn add_black(&mut self, parent: &mut BaseMut<'_, Field>) {
        let mut node = ColorRect::new_alloc();
        node.set_color(Color::from_rgb(0.0, 0.0, 0.0));
        node.set_size(Vector2::new(VIEW_WIDTH + 2.0, VIEW_HEIGHT + 2.0));
        node.set_z_index(89);
        parent.add_child(&node);
        self.visuals.push(Visual {
            node: VisualNode::Rect(node),
            offset: Vector2::new(-VIEW_WIDTH / 2.0 - 1.0, -VIEW_HEIGHT / 2.0 - 1.0),
            surface: Surface::Black,
            replay: BTreeMap::new(),
            press_cycle: Vec::new(),
        });
    }

    fn add_image(
        &mut self,
        parent: &mut BaseMut<'_, Field>,
        assets: &PackAssets,
        relative: &str,
        surface: Surface,
        centered: bool,
        z: i32,
    ) -> Option<()> {
        let image = assets.load_image(relative).or_else(|| {
            godot_error!("title: could not load image {relative}");
            None
        })?;
        let texture = ImageTexture::create_from_image(&image).or_else(|| {
            godot_error!("title: could not texture image {relative}");
            None
        })?;
        let press_cycle = if matches!(surface, Surface::PressStart) {
            press_start_cycle(&image)?
        } else {
            Vec::new()
        };
        let mut node = Sprite2D::new_alloc();
        let background = matches!(surface, Surface::Background | Surface::BackgroundTransfer);
        node.set_centered(if background { false } else { centered });
        node.set_texture(&texture);
        node.set_z_index(z);
        parent.add_child(&node);
        let offset = match surface {
            Surface::Background => screen_cell(0, 0),
            Surface::BackgroundTransfer => screen_cell(6, 0),
            Surface::SegaLogo => screen_cell(12, 11),
            Surface::TitleLogo => screen_cell(11, 3),
            Surface::Subtitle => screen_cell(5, 17),
            Surface::PressStart => screen_cell(11, 22),
            Surface::Copyright => screen_cell(12, 25),
            Surface::Black | Surface::Menu { .. } | Surface::SoundTestTrack { .. } => Vector2::ZERO,
        };
        let mut replay = BTreeMap::new();
        if !matches!(
            surface,
            Surface::Black | Surface::Menu { .. } | Surface::SoundTestTrack { .. }
        ) {
            let file = Path::new(relative)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            for frame in &self.replay_frames {
                let rel = format!("title/replay/frame_{frame}/{file}");
                if let Some(texture) = assets.load_texture(&rel) {
                    replay.insert(*frame, texture);
                }
            }
        }
        self.visuals.push(Visual {
            node: VisualNode::Sprite(node),
            offset,
            surface,
            replay,
            press_cycle,
        });
        Some(())
    }

    fn add_menu_art(
        &mut self,
        parent: &mut BaseMut<'_, Field>,
        assets: &PackAssets,
        set: &DialogueSet,
    ) -> bool {
        let Some(window) = assets.load_image(&set.window.png) else {
            godot_error!("title: could not load window art {}", set.window.png);
            return false;
        };
        let mut roles = HashMap::new();
        for role in Role::ALL {
            let Some(tile) = set.window.role(role) else {
                continue;
            };
            let Some(mut image) = window.get_region(Rect2i::new(
                Vector2i::new(tile.x, tile.y),
                Vector2i::new(tile.width as i32, tile.height as i32),
            )) else {
                continue;
            };
            if tile.flip_h {
                image.flip_x();
            }
            if tile.flip_v {
                image.flip_y();
            }
            if let Some(texture) = ImageTexture::create_from_image(&image) {
                roles.insert(role.as_str(), texture);
            }
        }
        self.add_frame(parent, &roles, MenuKind::NoSave, (13, 12, 14, 3));
        self.add_frame(parent, &roles, MenuKind::SaveOptions, (13, 10, 14, 7));
        self.add_frame(parent, &roles, MenuKind::Slots, (13, 10, 14, 7));
        self.add_frame(parent, &roles, MenuKind::EraseConfirm, (11, 10, 18, 6));

        let Some(font) = assets.load_image("dialogue/menu_font.png") else {
            godot_error!("title: could not load menu font dialogue/menu_font.png");
            return false;
        };
        let Some(font_texture) = ImageTexture::create_from_image(&font) else {
            godot_error!("title: could not texture menu font dialogue/menu_font.png");
            return false;
        };
        self.add_text(
            parent,
            &font_texture,
            MenuKind::NoSave,
            "START",
            (16, 13),
            0,
        );
        self.add_text(
            parent,
            &font_texture,
            MenuKind::SaveOptions,
            "CONTINUE",
            (16, 11),
            0,
        );
        self.add_text(
            parent,
            &font_texture,
            MenuKind::SaveOptions,
            "START",
            (16, 13),
            1,
        );
        self.add_text(
            parent,
            &font_texture,
            MenuKind::SaveOptions,
            "ERASE DATA",
            (16, 15),
            2,
        );
        for slot in 0..3 {
            self.add_text(
                parent,
                &font_texture,
                MenuKind::Slots,
                &format!("SLOT {}", slot + 1),
                (16, 11 + slot as i32 * 2),
                slot,
            );
        }
        self.add_text(
            parent,
            &font_texture,
            MenuKind::EraseConfirm,
            "ARE YOU SURE?",
            (13, 12),
            0,
        );
        self.add_text(
            parent,
            &font_texture,
            MenuKind::EraseConfirm,
            "YES",
            (17, 14),
            0,
        );
        self.add_text(
            parent,
            &font_texture,
            MenuKind::EraseConfirm,
            "NO",
            (23, 14),
            1,
        );
        self.add_sound_test_art(parent, assets, &roles, &font_texture);
        true
    }

    fn add_sound_test_art(
        &mut self,
        parent: &mut BaseMut<'_, Field>,
        assets: &PackAssets,
        roles: &HashMap<&'static str, Gd<ImageTexture>>,
        font_texture: &Gd<ImageTexture>,
    ) {
        self.add_frame(parent, roles, MenuKind::SoundTest, (9, 2, 22, 4));
        self.add_text(
            parent,
            font_texture,
            MenuKind::SoundTest,
            "PRESS START TO EXIT",
            (10, 3),
            usize::MAX,
        );
        self.add_frame(parent, roles, MenuKind::SoundTest, (3, 18, 34, 4));
        self.add_text(
            parent,
            font_texture,
            MenuKind::SoundTest,
            "A:PLAY   B:STOP   C:FADE",
            (8, 19),
            usize::MAX,
        );
        self.add_frame(parent, roles, MenuKind::SoundTest, (1, 9, 38, 6));
        let Ok(sound_test) = psiv_data::SoundTestFile::load_overlay(assets.overlay()) else {
            return;
        };
        for track in &sound_test.tracks {
            let number = format!("< {:02} >", track.index + 1);
            self.add_sound_test_text(parent, font_texture, &number, (2, 10), track.index);
            self.add_sound_test_text(parent, font_texture, &track.title, (4, 11), track.index);
            self.add_sound_test_text(parent, font_texture, &track.composer, (27, 13), track.index);
        }
    }

    fn add_sound_test_text(
        &mut self,
        parent: &mut BaseMut<'_, Field>,
        texture: &Gd<ImageTexture>,
        text: &str,
        cell: (i32, i32),
        track: usize,
    ) {
        for (column, character) in text.chars().enumerate() {
            if let Some(index) = menu_glyph_index(character) {
                let mut node = Sprite2D::new_alloc();
                node.set_centered(false);
                node.set_texture(texture);
                node.set_region_enabled(true);
                node.set_region_rect(Rect2::new(
                    Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
                    Vector2::new(CELL, CELL),
                ));
                node.set_z_index(93);
                parent.add_child(&node);
                self.visuals.push(Visual {
                    node: VisualNode::Sprite(node),
                    offset: Vector2::new(
                        (cell.0 + column as i32) as f32 * CELL - SCREEN_WIDTH / 2.0,
                        cell.1 as f32 * CELL - SCREEN_HEIGHT / 2.0,
                    ),
                    surface: Surface::SoundTestTrack { track },
                    replay: BTreeMap::new(),
                    press_cycle: Vec::new(),
                });
            }
        }
    }

    fn add_frame(
        &mut self,
        parent: &mut BaseMut<'_, Field>,
        roles: &HashMap<&'static str, Gd<ImageTexture>>,
        kind: MenuKind,
        rect: (i32, i32, i32, i32),
    ) {
        let (x, y, width, height) = rect;
        let mut add = |name: &'static str, cell_x: i32, cell_y: i32| {
            let Some(texture) = roles.get(name) else {
                return;
            };
            self.add_menu_sprite(
                parent,
                texture,
                kind,
                None,
                Vector2::new(
                    (cell_x as f32 * CELL) - SCREEN_WIDTH / 2.0,
                    (cell_y as f32 * CELL) - SCREEN_HEIGHT / 2.0,
                ),
            );
        };
        for row in 1..height - 1 {
            for column in 1..width - 1 {
                add("fill", x + column, y + row);
            }
        }
        for column in 1..width - 1 {
            add("edge_top", x + column, y);
            add("edge_bottom", x + column, y + height - 1);
        }
        for row in 1..height - 1 {
            add("edge_left", x, y + row);
            add("edge_right", x + width - 1, y + row);
        }
        add("corner_top_left", x, y);
        add("corner_top_right", x + width - 1, y);
        add("corner_bottom_left", x, y + height - 1);
        add("corner_bottom_right", x + width - 1, y + height - 1);
    }

    fn add_menu_sprite(
        &mut self,
        parent: &mut BaseMut<'_, Field>,
        texture: &Gd<ImageTexture>,
        kind: MenuKind,
        option: Option<usize>,
        offset: Vector2,
    ) {
        let mut node = Sprite2D::new_alloc();
        node.set_centered(false);
        node.set_texture(texture);
        node.set_z_index(92);
        parent.add_child(&node);
        self.visuals.push(Visual {
            node: VisualNode::Sprite(node),
            offset,
            surface: Surface::Menu { kind, option },
            replay: BTreeMap::new(),
            press_cycle: Vec::new(),
        });
    }

    fn add_text(
        &mut self,
        parent: &mut BaseMut<'_, Field>,
        texture: &Gd<ImageTexture>,
        kind: MenuKind,
        text: &str,
        cell: (i32, i32),
        option: usize,
    ) {
        for (column, character) in text.chars().enumerate() {
            if let Some(index) = menu_glyph_index(character) {
                let mut node = Sprite2D::new_alloc();
                node.set_centered(false);
                node.set_texture(texture);
                node.set_region_enabled(true);
                node.set_region_rect(Rect2::new(
                    Vector2::new((index % 16) as f32 * CELL, (index / 16) as f32 * CELL),
                    Vector2::new(CELL, CELL),
                ));
                node.set_z_index(93);
                parent.add_child(&node);
                self.visuals.push(Visual {
                    node: VisualNode::Sprite(node),
                    offset: Vector2::new(
                        (cell.0 + column as i32) as f32 * CELL - SCREEN_WIDTH / 2.0,
                        cell.1 as f32 * CELL - SCREEN_HEIGHT / 2.0,
                    ),
                    surface: Surface::Menu {
                        kind,
                        option: Some(option),
                    },
                    replay: BTreeMap::new(),
                    press_cycle: Vec::new(),
                });
            }
        }
    }

    pub(crate) fn reposition(&mut self, center: Vector2) {
        for visual in &mut self.visuals {
            let position = center + visual.offset;
            match &mut visual.node {
                VisualNode::Sprite(node) => node.set_position(position),
                VisualNode::Rect(node) => node.set_position(position),
            }
        }
    }

    /// One frame's view: the phases, the fade ramps, the CRAM replay and the
    /// prompt's cycle. The flow itself is the runtime's (`TitleView`); this is
    /// only what it looks like.
    pub(crate) fn present(&mut self, view: &TitleView) {
        let menu_kind = menu_kind(view);
        let reveal = view.phase == TitlePhase::Reveal;
        let title_art_ready = !reveal || view.ticks >= TITLE_ART_APPEAR_TICKS;
        let sega_alpha = (view.ticks as f32 / 75.0).min(1.0);
        let press_alpha = 1.0;
        for visual in &mut self.visuals {
            let (visible, alpha, selected) = match visual.surface {
                Surface::Black => (true, 1.0, false),
                Surface::SegaLogo => (view.phase == TitlePhase::Sega, sega_alpha, false),
                Surface::Background => (
                    view.phase != TitlePhase::Sega
                        && view.phase != TitlePhase::SoundTest
                        && !(reveal && !title_art_ready),
                    1.0,
                    false,
                ),
                Surface::BackgroundTransfer => (reveal && !title_art_ready, 1.0, false),
                Surface::TitleLogo | Surface::Subtitle => (
                    (reveal && title_art_ready) || view.phase == TitlePhase::PressStart,
                    1.0,
                    false,
                ),
                Surface::PressStart => (view.phase == TitlePhase::PressStart, press_alpha, false),
                Surface::Copyright => (view.phase == TitlePhase::PressStart, 1.0, false),
                Surface::Menu { kind, option } => (
                    menu_kind == Some(kind),
                    1.0,
                    option.is_some_and(|index| index == view.cursor),
                ),
                Surface::SoundTestTrack { track } => (
                    view.phase == TitlePhase::SoundTest && view.cursor == track,
                    1.0,
                    false,
                ),
            };
            match &mut visual.node {
                VisualNode::Sprite(node) => {
                    if let Some(frame) = self
                        .replay_frames
                        .iter()
                        .copied()
                        .take_while(|frame| *frame <= view.elapsed)
                        .last()
                        && let Some(texture) = visual.replay.get(&frame)
                    {
                        node.set_texture(texture);
                    }
                    if visible
                        && matches!(visual.surface, Surface::PressStart)
                        && let Some(texture) = visual
                            .press_cycle
                            .get(press_start_palette_index(view.elapsed))
                    {
                        node.set_texture(texture);
                    }
                    node.set_visible(visible);
                    node.set_modulate(if selected {
                        Color::from_rgba(1.0, 1.0, 0.35, alpha)
                    } else {
                        Color::from_rgba(1.0, 1.0, 1.0, alpha)
                    });
                }
                VisualNode::Rect(node) => {
                    let blue = if view.phase == TitlePhase::Sega {
                        if view.ticks <= 50 {
                            0.545 + 0.13 * view.ticks as f32 / 50.0
                        } else if view.ticks < 75 {
                            0.675 * (75 - view.ticks) as f32 / 25.0
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };
                    node.set_color(Color::from_rgb(0.0, 0.0, blue));
                    node.set_visible(visible);
                }
            }
        }
    }

    pub(crate) fn hide(&mut self) {
        for visual in &mut self.visuals {
            match &mut visual.node {
                VisualNode::Sprite(node) => node.set_visible(false),
                VisualNode::Rect(node) => node.set_visible(false),
            }
        }
    }

    pub(crate) fn dispose(mut self) {
        self.hide();
        for visual in &mut self.visuals {
            match &mut visual.node {
                VisualNode::Sprite(node) => node.queue_free(),
                VisualNode::Rect(node) => node.queue_free(),
            }
        }
    }
}

/// The window a [`TitleView`] shows, in this file's own surface names.
fn menu_kind(view: &TitleView) -> Option<MenuKind> {
    match view.window {
        TitleWindow::None => None,
        TitleWindow::NoSave => Some(MenuKind::NoSave),
        TitleWindow::SaveOptions => Some(MenuKind::SaveOptions),
        TitleWindow::Slots => Some(MenuKind::Slots),
        TitleWindow::EraseConfirm => Some(MenuKind::EraseConfirm),
        TitleWindow::SoundTest => Some(MenuKind::SoundTest),
    }
}

fn press_start_cycle(image: &Gd<Image>) -> Option<Vec<Gd<ImageTexture>>> {
    const BASE_PROMPT: (u8, u8, u8) = (238, 238, 65);
    let mut textures = Vec::with_capacity(PRESS_START_PALETTE.len());
    for word in PRESS_START_PALETTE {
        let mut frame = image.duplicate_resource();
        let color = title_cram_color(word);
        for y in 0..frame.get_height() {
            for x in 0..frame.get_width() {
                let pixel = frame.get_pixel(x, y);
                if pixel.a8() != 0 && (pixel.r8(), pixel.g8(), pixel.b8()) == BASE_PROMPT {
                    frame.set_pixel(x, y, color.with_alpha(f32::from(pixel.a8()) / 255.0));
                }
            }
        }
        textures.push(ImageTexture::create_from_image(&frame)?);
    }
    Some(textures)
}

/// Mirrors `DoPressStartButtonCyclingPal`: update only on every fourth main
/// frame, then select a word through the `$7C`/`>>1` byte offset. The table is
/// word-aligned, so its word index is the equivalent `>>2` expression.
fn press_start_palette_index(title_frame: u32) -> usize {
    let main_frame = title_frame.saturating_sub(TITLE_MAIN_FRAME_OFFSET);
    ((main_frame & 0x7C) >> 2) as usize
}

fn title_cram_color(word: u16) -> Color {
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

fn menu_glyph_index(character: char) -> Option<usize> {
    match character {
        'A'..='Z' => Some(character as usize - 'A' as usize),
        '0'..='9' => Some(26 + character as usize - '0' as usize),
        'a'..='z' => Some(56 + character as usize - 'a' as usize),
        '-' => Some(48),
        '!' => Some(49),
        '?' => Some(50),
        ':' => Some(51),
        ',' => Some(52),
        '.' => Some(53),
        '<' => Some(54),
        '>' => Some(55),
        ' ' => None,
        _ => None,
    }
}

fn screen_cell(x: i32, y: i32) -> Vector2 {
    Vector2::new(
        x as f32 * CELL - SCREEN_WIDTH / 2.0,
        y as f32 * CELL - SCREEN_HEIGHT / 2.0,
    )
}

fn load_replay_frames(assets: &PackAssets) -> Vec<u32> {
    let rel = "title/palette_cycle.json";
    let Ok(raw) = assets.read_text(rel) else {
        godot_warn!("title: CRAM replay manifest missing: {rel}");
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<TitleReplayManifest>(&raw) else {
        godot_error!("title: CRAM replay manifest is invalid: {rel}");
        return Vec::new();
    };
    let mut frames: Vec<u32> = manifest
        .frames
        .keys()
        .filter_map(|frame| frame.parse().ok())
        .collect();
    frames.sort_unstable();
    frames
}

/// `PSIV_DEBUG_TITLE_SHOT=1` with `PSIV_DEBUG_SHOT`: the deterministic title
/// capture at its pinned clone tick. The phases it lands on are the runtime's
/// (`TitleView::phase`); the log line carries the same two counters as before
/// S5 moved the flow.
pub(crate) fn title_debug_shot(field: &Field, anim_tick: u64, title_ticks: u32) {
    let Ok(path) = std::env::var("PSIV_DEBUG_SHOT") else {
        return;
    };
    let at: u64 = std::env::var("PSIV_DEBUG_SHOT_FRAME")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(180);
    if anim_tick != at || !std::env::var("PSIV_DEBUG_TITLE_SHOT").is_ok_and(|value| value == "1") {
        return;
    }
    let Some(viewport) = field.base().get_viewport() else {
        return;
    };
    let Some(texture) = viewport.get_texture() else {
        return;
    };
    let Some(image) = texture.get_image() else {
        return;
    };
    let error = image.save_png(&GString::from(path.as_str()));
    godot_print!(
        "debug: title screenshot -> {path} ({error:?}, anim_tick={anim_tick}, title_ticks={title_ticks})"
    );
}

#[cfg(test)]
mod tests {
    use super::press_start_palette_index;

    #[test]
    fn press_start_cycle_uses_retail_main_frame_phase() {
        assert_eq!(press_start_palette_index(500), 5);
        assert_eq!(press_start_palette_index(550), 18);
        assert_eq!(press_start_palette_index(600), 30);
    }
}
