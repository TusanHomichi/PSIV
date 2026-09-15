//! Retail battle status-strip icons.
//!
//! Kept separate from the command surface so the battle UI module stays small
//! enough to remain reviewable while the vehicle menu grows.

use godot::prelude::*;

use super::state::PartyStatus;
use super::ui::{BATTLE_CELL_PIXELS, STATUS_NAME_Y, STATUS_PANE_START_CELLS};

const QUESTION: [&str; 16] = [
    "BWWWWWWWWWWWWWWB",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKqoYYYYoqKKKW",
    "WKKKoYqKKqYoqKKW",
    "WKKKYYKKKKYYqKKW",
    "WKKKoYKKKKYYqKKW",
    "WKKKKKKqoYYqKKKW",
    "WKKKKKKYYoqKKKKW",
    "WKKKKKKYYqKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKYYqKKKKKW",
    "WKKKKKKYYqKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "BWWWWWWWWWWWWWWB",
];

const BLANK: [&str; 16] = [
    "BWWWWWWWWWWWWWWB",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "WKKKKKKKKKKKKKKW",
    "BWWWWWWWWWWWWWWB",
];

pub(super) fn status_pixels(
    palette: [Color; 16],
    party_status: &[PartyStatus],
) -> Vec<(Vector2, Color)> {
    let mut pixels = Vec::new();
    for (pane, start) in STATUS_PANE_START_CELLS.iter().copied().enumerate() {
        let member = super::layout::status_member(party_status, pane).is_some();
        {
            let origin = Vector2::new(
                (start + 5) as f32 * BATTLE_CELL_PIXELS as f32,
                STATUS_NAME_Y,
            );
            pixels.extend(pattern_pixels(
                origin,
                if member { &QUESTION } else { &BLANK },
                palette,
            ));
        }
    }
    pixels
}

fn pattern_pixels(origin: Vector2, rows: &[&str], palette: [Color; 16]) -> Vec<(Vector2, Color)> {
    let mut pixels = Vec::new();
    for (row, line) in rows.iter().enumerate() {
        for (column, symbol) in line.chars().enumerate() {
            let color = match symbol {
                'B' => palette[14],
                'W' => palette[15],
                'K' => palette[0],
                'L' => palette[2],
                'R' => palette[13],
                'P' => palette[12],
                'Y' => palette[11],
                'q' => palette[6],
                'o' => palette[5],
                _ => continue,
            };
            pixels.push((
                Vector2::new(origin.x + column as f32, origin.y + row as f32),
                color,
            ));
        }
    }
    pixels
}
