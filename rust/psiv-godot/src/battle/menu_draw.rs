//! Drawing for the command window: the title, the rows and the footer.
//!
//! The compact native menu uses the extracted window/font; its layout is not
//! yet a certified reproduction of the cartridge's character command windows,
//! which are a horizontal five-icon strip (`Battle_CharCommand`, `ps4.asm:2192`,
//! places its cursor at `base + index * 24` pixels). What the window *says*
//! comes from the runtime's `CommandMenuView`; this file only places it.

use godot::prelude::Rect2;
use godot::prelude::Vector2;

use psiv_runtime::{CommandMenuView, MenuPage};

use super::chrome::{BattleChrome, Quad, WindowRect};

/// Draws the window and its rows.
pub(super) fn draw(chrome: &BattleChrome, menu: &CommandMenuView, quads: &mut Vec<Quad>) {
    let rows = &menu.rows;
    let page_size = if menu.page == MenuPage::Actions { 5 } else { 4 };
    let rect = WindowRect {
        x: 96.0,
        y: 8.0,
        w: 208.0,
        h: 48.0 + 16.0 * rows.len().min(page_size) as f32,
    };
    if let Some(frame) = chrome.frame(rect) {
        quads.extend(frame);
    }
    quads.extend(chrome.text(
        &menu.title,
        WindowRect {
            x: 104.0,
            y: 16.0,
            w: 192.0,
            h: 8.0,
        },
    ));
    let first = menu.cursor / page_size * page_size;
    for (i, row) in rows.iter().enumerate().skip(first).take(page_size) {
        let y = 32.0 + (i - first) as f32 * 16.0;
        let pattern = if i == menu.cursor { 0x6e8 } else { 0x6e7 };
        if let Some(quad) = chrome.window_word(
            pattern,
            false,
            false,
            Rect2::new(Vector2::new(104.0, y), Vector2::new(8.0, 8.0)),
        ) {
            quads.push(quad);
        }
        let text = format!("{}{}", if row.enabled { "" } else { "-" }, row.label);
        quads.extend(chrome.text(
            &text,
            WindowRect {
                x: 120.0,
                y,
                w: 176.0,
                h: 8.0,
            },
        ));
    }
    let footer = if rows.len() > page_size {
        format!(
            "PAGE {} OF {} CANCEL: BACK",
            first / page_size + 1,
            rows.len().div_ceil(page_size)
        )
    } else {
        "CANCEL: BACK".into()
    };
    quads.extend(chrome.text(
        &footer,
        WindowRect {
            x: 104.0,
            y: rect.y + rect.h - 16.0,
            w: 192.0,
            h: 8.0,
        },
    ));
}
