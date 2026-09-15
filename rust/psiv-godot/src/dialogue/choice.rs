//! Retail yes/no window, and the input that supplies an actual answer.
use super::{DialogueWindow, Quad, WindowView, load_image};
use godot::classes::{ImageTexture, Input};
use godot::prelude::*;
use psiv_data::{Ctrl, DialogueEntry, DialogueSet, Role, Segment};

pub(super) struct ChoiceView {
    font: Gd<ImageTexture>,
    cursor: Gd<ImageTexture>,
    origin: Vector2,
    cells: (i32, i32),
}

impl ChoiceView {
    pub(super) fn build(pack: &str, set: &DialogueSet) -> Option<Self> {
        let record = set.window.windows.iter().find(|w| w.name == "yes_no")?;
        let text = set.window.text_window.rect;
        let strip = load_image(pack, &set.window.png)?;
        // ArtNem_Window pattern $6E8, relative to its $680 VRAM base.
        let cursor =
            strip.get_region(Rect2i::new(Vector2i::new(0x68 * 8, 0), Vector2i::new(8, 8)))?;
        Some(Self {
            font: ImageTexture::create_from_image(&load_image(pack, "dialogue/menu_font.png")?)?,
            cursor: ImageTexture::create_from_image(&cursor)?,
            origin: Vector2::new(
                (record.rect.x - text.x) as f32,
                (record.rect.y - text.y) as f32,
            ),
            cells: (record.width_cells as i32, record.height_cells as i32),
        })
    }

    pub(super) fn quads(&self, view: &WindowView, selected: usize) -> Vec<Quad> {
        let mut out = Vec::new();
        let cell = Vector2::new(8.0, 8.0);
        for y in 0..self.cells.1 {
            for x in 0..self.cells.0 {
                let role = match (x == 0, x == self.cells.0 - 1, y == 0, y == self.cells.1 - 1) {
                    (true, _, true, _) => Role::CornerTopLeft,
                    (_, true, true, _) => Role::CornerTopRight,
                    (true, _, _, true) => Role::CornerBottomLeft,
                    (_, true, _, true) => Role::CornerBottomRight,
                    (_, _, true, _) => Role::EdgeTop,
                    (_, _, _, true) => Role::EdgeBottom,
                    (true, _, _, _) => Role::EdgeLeft,
                    (_, true, _, _) => Role::EdgeRight,
                    _ => Role::Fill,
                };
                if let Some(texture) = view.tile(role) {
                    out.push(Quad {
                        texture: texture.clone(),
                        dest: Rect2::new(
                            self.origin + Vector2::new(x as f32 * 8.0, y as f32 * 8.0),
                            cell,
                        ),
                        src: Rect2::new(Vector2::ZERO, cell),
                    });
                }
            }
        }
        for (row, label) in ["YES", "NO"].iter().enumerate() {
            for (column, ch) in label.bytes().enumerate() {
                let glyph = ch - b'A';
                out.push(Quad {
                    texture: self.font.clone(),
                    dest: Rect2::new(
                        self.origin
                            + Vector2::new(16.0 + column as f32 * 8.0, 8.0 + row as f32 * 16.0),
                        cell,
                    ),
                    src: Rect2::new(
                        Vector2::new(f32::from(glyph % 16) * 8.0, f32::from(glyph / 16) * 8.0),
                        cell,
                    ),
                });
            }
        }
        out.push(Quad {
            texture: self.cursor.clone(),
            dest: Rect2::new(
                self.origin + Vector2::new(8.0, 8.0 + selected as f32 * 16.0),
                cell,
            ),
            src: Rect2::new(Vector2::ZERO, cell),
        });
        out
    }
}

impl DialogueWindow {
    pub(crate) fn debug_choice(&self) -> Option<serde_json::Value> {
        let flow = self.flow.as_ref().filter(|flow| flow.has_choice())?;
        Some(
            serde_json::json!({"ready":self.choice_ready(),"cursor":self.choice_cursor,"lines":flow.lines()}),
        )
    }

    pub(super) fn choice_ready(&self) -> bool {
        !self.is_opening()
            && self.flow.as_ref().is_some_and(|flow| {
                flow.has_choice()
                    && self.revealed >= flow.lines().iter().map(|s| s.chars().count()).sum()
            })
    }

    /// Choice input owns the prompt, including while it finishes typing.
    /// Cancel is retail's direct NO shortcut; merely advancing never picks.
    pub fn handle_choice_input(&mut self) -> bool {
        if !self.flow.as_ref().is_some_and(|flow| flow.has_choice()) {
            return false;
        }
        if !self.choice_ready() {
            return true;
        }
        let input = Input::singleton();
        if input.is_action_just_pressed("ui_up") || input.is_action_just_pressed("ui_down") {
            self.choice_cursor ^= 1;
            self.base_mut().queue_redraw();
        }
        let answer = if input.is_action_just_pressed("ui_cancel") {
            Some(false)
        } else if input.is_action_just_pressed("ui_accept") {
            Some(self.choice_cursor == 0)
        } else {
            None
        };
        if let Some(yes) = answer {
            godot_print!("dialogue choice: {}", if yes { "YES" } else { "NO" });
            self.pending_choice = Some(yes);
            if self.standalone_choice {
                self.close();
                self.standalone_choice = false;
            } else {
                if let Some(flow) = self.flow.as_mut() {
                    flow.answer_choice(yes);
                }
                self.revealed = 0;
                self.reveal_tick = 0;
                self.choice_cursor = 0;
                self.service_flow_signals();
                self.sync_portrait();
                self.base_mut().queue_redraw();
            }
        }
        true
    }

    pub fn take_pending_choice(&mut self) -> Option<bool> {
        self.pending_choice.take()
    }

    /// A scene branch without a preceding text choice remains interactive.
    pub fn open_scene_choice(&mut self) -> bool {
        let entry = DialogueEntry {
            id: 0,
            text: String::new(),
            pages: Vec::new(),
            segments: vec![Segment::Control(Ctrl::YesNo {
                code: 0xF5,
                operands: vec![0, 0],
                yes_entry: 0,
                no_entry: 0,
            })],
        };
        let opened = self.open(&entry);
        self.scene_dialogue = true;
        self.standalone_choice = true;
        opened
    }
}
