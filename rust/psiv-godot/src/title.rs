//! Retail front-door presentation: Sega logo, title art, and save menu.
//!
//! The title surface is intentionally presentation-only.  Its five art
//! placements and the two menu windows are the decoded oracle constants in
//! `oracle/decode_layout.py`; save validation stays in `boot.rs`, and choosing
//! START releases the already-authoritative new-game runtime.  The opening
//! scene's plane/text operations are a separate renderer slice.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use godot::classes::{ColorRect, Image, ImageTexture, Sprite2D};
use godot::obj::BaseMut;
use godot::prelude::*;
use psiv_core::Input as CoreInput;
use psiv_data::{DialogueSet, Role};
use psiv_runtime::Runtime;

use super::{Field, StepFrames, TransitionKind, read_input, save_directory};

const SCREEN_WIDTH: f32 = 320.0;
const SCREEN_HEIGHT: f32 = 224.0;
const CAMERA_ZOOM: f32 = 3.0;
const VIEW_WIDTH: f32 = 1280.0 / CAMERA_ZOOM;
const VIEW_HEIGHT: f32 = 800.0 / CAMERA_ZOOM;
const CELL: f32 = 8.0;

// Oracle tape 25: Sega holds through frame 200; the decoded title mappings
// settle by roughly frame 450, and the retail press-start routine waits 0x233
// (563) frames before opening the menu.
const SEGA_HOLD_TICKS: u32 = 200;
const TITLE_ART_APPEAR_TICKS: u32 = 248;
const TITLE_REVEAL_TICKS: u32 = 299;
const PRESS_START_HOLD_TICKS: u32 = 563;

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
enum Phase {
    Sega,
    Reveal,
    PressStart,
    Menu,
    Slots,
    EraseSlots,
    EraseConfirm,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuKind {
    NoSave,
    SaveOptions,
    Slots,
    EraseConfirm,
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

#[derive(Clone, Copy)]
enum TitleChoice {
    Start,
    Continue(usize),
    Erase(usize),
}

/// State and nodes for the front door.  The nodes remain children of Field so
/// the Godot scene stays untouched; hiding them hands control back to the
/// existing field shell without changing scenes or save serialization.
pub(crate) struct TitleScreen {
    visuals: Vec<Visual>,
    slots: [bool; 3],
    phase: Phase,
    ticks: u32,
    menu_index: usize,
    erase_slot: usize,
    accept_down: bool,
    direction_down: bool,
    replay_frames: Vec<u32>,
    elapsed: u32,
}

impl TitleScreen {
    pub(crate) fn build(
        pack_dir: &str,
        slots: [bool; 3],
        mut parent: BaseMut<'_, Field>,
    ) -> Option<TitleScreen> {
        let mut screen = TitleScreen {
            visuals: Vec::new(),
            slots,
            phase: Phase::Sega,
            ticks: 0,
            menu_index: 0,
            erase_slot: 0,
            accept_down: false,
            direction_down: false,
            replay_frames: load_replay_frames(pack_dir),
            elapsed: 0,
        };

        let dialogue = match DialogueSet::load(std::path::Path::new(pack_dir)) {
            Ok(set) => set,
            Err(error) => {
                godot_error!("title: could not load dialogue pack: {error}");
                return None;
            }
        };
        screen.add_black(&mut parent);
        screen.add_image(
            &mut parent,
            pack_dir,
            "title/background.png",
            Surface::Background,
            true,
            90,
        )?;
        screen.add_image(
            &mut parent,
            pack_dir,
            "title/titlebarbgtoppart.png",
            Surface::BackgroundTransfer,
            false,
            90,
        )?;
        screen.add_image(
            &mut parent,
            pack_dir,
            "title/sega_logo.png",
            Surface::SegaLogo,
            false,
            91,
        )?;
        screen.add_image(
            &mut parent,
            pack_dir,
            "title/title_logo.png",
            Surface::TitleLogo,
            false,
            91,
        )?;
        screen.add_image(
            &mut parent,
            pack_dir,
            "title/subtitle.png",
            Surface::Subtitle,
            false,
            91,
        )?;
        screen.add_image(
            &mut parent,
            pack_dir,
            "title/press_start.png",
            Surface::PressStart,
            false,
            91,
        )?;
        screen.add_image(
            &mut parent,
            pack_dir,
            "title/copyright.png",
            Surface::Copyright,
            false,
            91,
        )?;

        if !screen.add_menu_art(&mut parent, pack_dir, &dialogue) {
            screen.hide();
            return None;
        }
        screen.apply_phase();
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
        pack_dir: &str,
        relative: &str,
        surface: Surface,
        centered: bool,
        z: i32,
    ) -> Option<()> {
        let path = format!("{pack_dir}/{relative}");
        let image = Image::load_from_file(&GString::from(path.as_str())).or_else(|| {
            godot_error!("title: could not load image {path}");
            None
        })?;
        let texture = ImageTexture::create_from_image(&image).or_else(|| {
            godot_error!("title: could not texture image {path}");
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
            Surface::Black | Surface::Menu { .. } => Vector2::ZERO,
        };
        let mut replay = BTreeMap::new();
        if !matches!(surface, Surface::Black | Surface::Menu { .. }) {
            let file = Path::new(relative)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            for frame in &self.replay_frames {
                let path = format!("{pack_dir}/title/replay/frame_{frame}/{file}");
                if let Some(image) = Image::load_from_file(&GString::from(path.as_str()))
                    && let Some(texture) = ImageTexture::create_from_image(&image)
                {
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
        pack_dir: &str,
        set: &DialogueSet,
    ) -> bool {
        let window_path = format!("{pack_dir}/{}", set.window.png);
        let Some(window) = Image::load_from_file(&GString::from(window_path.as_str())) else {
            godot_error!("title: could not load window art {window_path}");
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

        let font_path = format!("{pack_dir}/dialogue/menu_font.png");
        let Some(font) = Image::load_from_file(&GString::from(font_path.as_str())) else {
            godot_error!("title: could not load menu font {font_path}");
            return false;
        };
        let Some(font_texture) = ImageTexture::create_from_image(&font) else {
            godot_error!("title: could not texture menu font {font_path}");
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
        true
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

    fn menu_kind(&self) -> Option<MenuKind> {
        match self.phase {
            Phase::Menu if self.slots.iter().any(|slot| *slot) => Some(MenuKind::SaveOptions),
            Phase::Menu => Some(MenuKind::NoSave),
            Phase::Slots => Some(MenuKind::Slots),
            Phase::EraseSlots => Some(MenuKind::Slots),
            Phase::EraseConfirm => Some(MenuKind::EraseConfirm),
            _ => None,
        }
    }

    fn apply_phase(&mut self) {
        let menu_kind = self.menu_kind();
        let title_art_ready = self.phase != Phase::Reveal || self.ticks >= TITLE_ART_APPEAR_TICKS;
        let sega_alpha = (self.ticks as f32 / 75.0).min(1.0);
        let press_alpha = 1.0;
        for visual in &mut self.visuals {
            let (visible, alpha, selected) = match visual.surface {
                Surface::Black => (true, 1.0, false),
                Surface::SegaLogo => (self.phase == Phase::Sega, sega_alpha, false),
                Surface::Background => (
                    self.phase != Phase::Sega && !(self.phase == Phase::Reveal && !title_art_ready),
                    1.0,
                    false,
                ),
                Surface::BackgroundTransfer => {
                    (self.phase == Phase::Reveal && !title_art_ready, 1.0, false)
                }
                Surface::TitleLogo | Surface::Subtitle => (
                    (self.phase == Phase::Reveal && title_art_ready)
                        || self.phase == Phase::PressStart,
                    1.0,
                    false,
                ),
                Surface::PressStart => (self.phase == Phase::PressStart, press_alpha, false),
                Surface::Copyright => (self.phase == Phase::PressStart, 1.0, false),
                Surface::Menu { kind, option } => (
                    menu_kind == Some(kind),
                    1.0,
                    option.is_some_and(|index| index == self.menu_index),
                ),
            };
            match &mut visual.node {
                VisualNode::Sprite(node) => {
                    if let Some(frame) = self
                        .replay_frames
                        .iter()
                        .copied()
                        .take_while(|frame| *frame <= self.elapsed)
                        .last()
                        && let Some(texture) = visual.replay.get(&frame)
                    {
                        node.set_texture(texture);
                    }
                    if visible
                        && matches!(visual.surface, Surface::PressStart)
                        && let Some(texture) = visual
                            .press_cycle
                            .get(press_start_palette_index(self.elapsed))
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
                    let blue = if self.phase == Phase::Sega {
                        if self.ticks <= 50 {
                            0.545 + 0.13 * self.ticks as f32 / 50.0
                        } else if self.ticks < 75 {
                            0.675 * (75 - self.ticks) as f32 / 25.0
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

    fn move_menu(&mut self, direction: psiv_core::Direction) {
        let count = match self.phase {
            Phase::Menu if self.slots.iter().any(|slot| *slot) => 3,
            Phase::Menu => 1,
            Phase::Slots | Phase::EraseSlots => 3,
            Phase::EraseConfirm => 2,
            _ => return,
        };
        self.menu_index = match direction {
            psiv_core::Direction::Up if self.menu_index == 0 => count - 1,
            psiv_core::Direction::Up => self.menu_index - 1,
            psiv_core::Direction::Down => (self.menu_index + 1) % count,
            _ => self.menu_index,
        };
    }

    fn accept(&mut self) -> Option<TitleChoice> {
        match self.phase {
            Phase::Sega => {
                self.phase = Phase::Reveal;
                self.ticks = 0;
            }
            Phase::Reveal => {
                self.phase = Phase::PressStart;
                self.ticks = 0;
            }
            Phase::PressStart => {
                self.phase = Phase::Menu;
                self.ticks = 0;
            }
            Phase::Menu if !self.slots.iter().any(|slot| *slot) => {
                return Some(TitleChoice::Start);
            }
            Phase::Menu if self.menu_index == 0 => {
                self.phase = Phase::Slots;
                self.menu_index = self.slots.iter().position(|slot| *slot).unwrap_or(0);
            }
            Phase::Menu if self.menu_index == 1 => return Some(TitleChoice::Start),
            Phase::Menu => {
                self.phase = Phase::EraseSlots;
                self.menu_index = self.slots.iter().position(|slot| *slot).unwrap_or(0);
            }
            Phase::Slots if self.slots.get(self.menu_index).copied().unwrap_or(false) => {
                return Some(TitleChoice::Continue(self.menu_index));
            }
            Phase::Slots => godot_print!("title: selected save slot is empty"),
            Phase::EraseSlots if self.slots.get(self.menu_index).copied().unwrap_or(false) => {
                self.erase_slot = self.menu_index;
                self.phase = Phase::EraseConfirm;
                self.menu_index = 0;
            }
            Phase::EraseSlots => godot_print!("title: selected save slot is empty"),
            Phase::EraseConfirm if self.menu_index == 0 => {
                return Some(TitleChoice::Erase(self.erase_slot));
            }
            Phase::EraseConfirm => {
                self.phase = Phase::EraseSlots;
                self.menu_index = self.erase_slot;
            }
        }
        None
    }

    fn tick(&mut self, input: CoreInput) -> Option<TitleChoice> {
        self.ticks = self.ticks.saturating_add(1);
        self.elapsed = self.elapsed.saturating_add(1);
        let accept_down = matches!(input, CoreInput::Action);
        let pressed = accept_down && !self.accept_down;
        self.accept_down = accept_down;
        let direction = match input {
            CoreInput::Direction(direction) => Some(direction),
            _ => None,
        };
        let direction_pressed = direction.is_some() && !self.direction_down;
        self.direction_down = direction.is_some();
        if direction_pressed && let Some(direction) = direction {
            self.move_menu(direction);
        }
        let choice = if pressed { self.accept() } else { None };
        if choice.is_none() {
            match self.phase {
                Phase::Sega if self.ticks > SEGA_HOLD_TICKS => {
                    self.phase = Phase::Reveal;
                    self.ticks = 0;
                }
                Phase::Reveal if self.ticks > TITLE_REVEAL_TICKS => {
                    self.phase = Phase::PressStart;
                    self.ticks = 0;
                }
                Phase::PressStart if self.ticks >= PRESS_START_HOLD_TICKS => {
                    self.phase = Phase::Menu;
                    self.ticks = 0;
                }
                _ => {}
            }
        }
        self.apply_phase();
        choice
    }

    pub(crate) fn hide(&mut self) {
        for visual in &mut self.visuals {
            match &mut visual.node {
                VisualNode::Sprite(node) => node.set_visible(false),
                VisualNode::Rect(node) => node.set_visible(false),
            }
        }
    }
}

fn press_start_cycle(image: &Gd<Image>) -> Option<Vec<Gd<ImageTexture>>> {
    const BASE_PROMPT: (u8, u8, u8) = (238, 238, 65);
    let mut textures = Vec::with_capacity(PRESS_START_PALETTE.len());
    for word in PRESS_START_PALETTE {
        let mut frame = image.duplicate()?.try_cast::<Image>().ok()?;
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

fn load_replay_frames(pack_dir: &str) -> Vec<u32> {
    let path = format!("{pack_dir}/title/palette_cycle.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        godot_warn!("title: CRAM replay manifest missing: {path}");
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<TitleReplayManifest>(&raw) else {
        godot_error!("title: CRAM replay manifest is invalid: {path}");
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

impl Field {
    /// Drives the title while starving the field runtime of input and ticks.
    pub(super) fn drive_title(&mut self) -> bool {
        let Some(mut title) = self.title.take() else {
            return false;
        };
        let choice = title.tick(read_input());
        if let Some(camera) = self.camera.as_ref() {
            title.reposition(camera.get_position());
        }
        title_debug_shot(self, self.anim_tick, title.ticks);
        if let Some(choice) = choice {
            match choice {
                TitleChoice::Erase(slot) => match Runtime::erase_slot(&save_directory(), slot) {
                    Ok(path) => {
                        godot_print!("title: erased save slot {} ({})", slot + 1, path.display());
                        title.slots[slot] = false;
                        title.phase = Phase::Menu;
                        title.menu_index = 0;
                        title.ticks = 0;
                        title.apply_phase();
                        self.title = Some(title);
                    }
                    Err(error) => {
                        godot_error!("title: ERASE DATA slot {} failed: {error}", slot + 1);
                        title.phase = Phase::EraseSlots;
                        title.menu_index = slot;
                        title.apply_phase();
                        self.title = Some(title);
                    }
                },
                choice => {
                    title.hide();
                    self.finish_title_choice(choice);
                }
            }
        } else {
            self.title = Some(title);
        }
        true
    }

    fn finish_title_choice(&mut self, choice: TitleChoice) {
        match choice {
            TitleChoice::Start => {
                godot_print!("title: START selected; handing off to the existing new-game runtime");
                self.start_transition(TransitionKind::GameStart);
            }
            TitleChoice::Continue(slot) => {
                let Some(data) = self.runtime.as_ref().map(|runtime| runtime.data().clone()) else {
                    godot_error!("title: CONTINUE selected without a runtime");
                    return;
                };
                match Runtime::load_slot(data, &save_directory(), slot, StepFrames::default()) {
                    Ok(mut runtime) => {
                        godot_print!("title: CONTINUE loaded slot {}", slot + 1);
                        self.configure_battles(&mut runtime);
                        self.runtime = Some(runtime);
                        self.load_map_visuals();
                        self.play_map_music();
                        self.sync_visuals(false);
                        self.start_transition(TransitionKind::GameStart);
                    }
                    Err(error) => godot_error!(
                        "title: CONTINUE slot {} failed validation: {error}",
                        slot + 1
                    ),
                }
            }
            TitleChoice::Erase(_) => unreachable!("title erase is completed by drive_title"),
        }
    }
}

fn title_debug_shot(field: &Field, anim_tick: u64, title_ticks: u32) {
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
