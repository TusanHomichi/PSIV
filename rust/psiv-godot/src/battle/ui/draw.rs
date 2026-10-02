//! One frame of plane-A composition from the runtime's view.
//!
//! Lifted out of the single `battle/ui.rs` file with the S3 node. Every window
//! rectangle, tile pattern and text origin here is a decoded oracle placement
//! (`docs/battle/BATTLE_GEOMETRY.md`, `oracle/layouts/*.json`); what the
//! windows *say* now comes from the runtime's `BattleView`.

use godot::builtin::PackedVector2Array;
use godot::prelude::*;

use psiv_runtime::{BattleView, MenuView, MessageKind};

use super::{
    BATTLE_CELL_PIXELS, BattleScreen, COMMAND_CURSOR_WORDS, COMMAND_RECT, COMMAND_TEXT_RECT,
    DAMAGE_HEIGHT, DAMAGE_WIDTH, ENEMY_DAMAGE_Y, ENEMY_NAME_RECT, ENEMY_NAME_TEXT_RECT,
    LONG_TRANSIENT_MESSAGE_RECT, PARTY_DAMAGE_Y, TRANSIENT_MESSAGE_RECT, VICTORY_RECT,
    VICTORY_TEXT_RECT, WIDE_MESSAGE_RECT,
};
use crate::battle::chrome::WindowRect;
use crate::battle::layout::{DEFAULT_TRANSIENT_COLUMN, append_status_quads, tile_dest};
use crate::battle::{layout, menu_draw, status, vehicle_ui};

/// Builds this frame's quads and paints them.
pub(super) fn compose(screen: &mut BattleScreen) {
    let mut quads = Vec::new();
    let mut icon_pixels = Vec::new();
    if let (Some(chrome), Some(view)) = (screen.chrome.as_ref(), screen.view.as_ref()) {
        let cell = chrome.cell;
        match view.menu.as_ref() {
            Some(MenuView::Top { cursor }) => {
                if let Some(frame) = chrome.frame(ENEMY_NAME_RECT) {
                    quads.extend(frame);
                    let name = first_visible_enemy(view).map_or("", String::as_str);
                    quads.extend(chrome.text(name, ENEMY_NAME_TEXT_RECT));
                }
                if let Some(frame) = chrome.frame(COMMAND_RECT) {
                    quads.extend(frame);
                    let labels = if screen.vehicle_index.is_some() {
                        "ATTAC\nOPTIN\nRUN"
                    } else {
                        "COMD\nMACR\nRUN"
                    };
                    quads.extend(chrome.text_with_pitch(labels, COMMAND_TEXT_RECT, 16.0));
                }
                for row in 0..COMMAND_CURSOR_WORDS.len() {
                    let pattern = if row == *cursor { 0x6e8 } else { 0x6e7 };
                    if let Some(quad) =
                        chrome.window_word(pattern, false, false, tile_dest(4, 6 + row as i32 * 2))
                    {
                        // `0x6e8` is the selected command; `0x6e7` is the
                        // retail blue disabled/unselected form for the others.
                        quads.push(quad);
                    }
                }
            }
            Some(MenuView::Commands(commands)) => menu_draw::draw(chrome, commands, &mut quads),
            Some(MenuView::VehicleSkills { slots, cursor }) => {
                vehicle_ui::draw(chrome, slots, *cursor, &mut quads);
            }
            None => {}
        }
        match view.message_kind {
            MessageKind::Transient | MessageKind::Wide if !view.message.is_empty() => {
                let rect = match view.message_kind {
                    MessageKind::Transient if view.message == "DEFENSE" => WindowRect {
                        x: transient_column(view) as f32 * cell,
                        ..TRANSIENT_MESSAGE_RECT
                    },
                    MessageKind::Transient => LONG_TRANSIENT_MESSAGE_RECT,
                    _ => WIDE_MESSAGE_RECT,
                };
                if let Some(frame) = chrome.frame(rect) {
                    quads.extend(frame);
                    quads.extend(chrome.text(&view.message, rect.inset(cell)));
                }
            }
            MessageKind::Victory => {
                quads.extend(chrome.victory_quads(VICTORY_RECT, VICTORY_TEXT_RECT, None));
            }
            MessageKind::VictoryRewards => {
                quads.extend(chrome.victory_quads(
                    VICTORY_RECT,
                    VICTORY_TEXT_RECT,
                    Some((view.reward_each, view.reward_meseta)),
                ));
            }
            MessageKind::None | MessageKind::Transient | MessageKind::Wide => {}
        }
        append_status_quads(chrome, &view.party, &mut quads);
        if let Some(damage) = view.damage
            && let Some(column) = screen.damage_column(damage.target)
        {
            // §4: enemy damage is row 5, party damage row 18; each block is 5x2.
            let rect = WindowRect {
                x: column as f32 * BATTLE_CELL_PIXELS as f32,
                y: if damage.target.side() == psiv_core::battle::Side::Enemy {
                    ENEMY_DAMAGE_Y
                } else {
                    PARTY_DAMAGE_Y
                },
                w: DAMAGE_WIDTH,
                h: DAMAGE_HEIGHT,
            };
            quads.extend(chrome.damage_quads(damage.amount, rect));
        }
        icon_pixels = status::status_pixels(chrome.palette, &view.party);
    }
    for quad in quads {
        screen
            .base_mut()
            .draw_texture_rect_region(&quad.texture, quad.dest, quad.src);
    }
    for (position, color) in icon_pixels {
        let points = PackedVector2Array::from(&[
            position,
            Vector2::new(position.x + 1.0, position.y),
            Vector2::new(position.x + 1.0, position.y + 1.0),
            Vector2::new(position.x, position.y + 1.0),
        ]);
        screen.base_mut().draw_colored_polygon(&points, color);
    }
}

/// The name window shows the first enemy the story still shows, in the order
/// the sprite nodes were built.
fn first_visible_enemy(view: &BattleView) -> Option<&String> {
    view.enemies
        .iter()
        .find(|enemy| enemy.visible)
        .map(|enemy| &enemy.name)
}

/// The transient line's column: the default unless the beat named a fighter —
/// the column table is a screen placement, so it stays here.
fn transient_column(view: &BattleView) -> i32 {
    view.transient
        .map_or(DEFAULT_TRANSIENT_COLUMN, layout::transient_column)
}
