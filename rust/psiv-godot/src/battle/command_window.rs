//! The per-character command strip and the technique, skill and item windows,
//! placed the way the cartridge writes them into plane A.
//!
//! What the windows *say* comes from the runtime's [`StripView`] and
//! [`ListView`]; this file only places it. Every position is a routine's
//! plane-map arithmetic, cited at the constant; `docs/battle/BATTLE_COMMAND_UI.md`
//! carries the decoded cells next to the oracle frames they were read from.

use psiv_runtime::{CommandMenuView, MenuPage};

use super::chrome::{BattleChrome, Quad, WindowRect};
use super::layout::tile_dest;
use super::tiles::{CURSOR_FIRST_TILE, CommandTiles};

/// First plane column of the strip for each acting slot. `loc_17A4`
/// (`ps4.asm:2410`) adds `$06, $1E, $12, $12, $1E` bytes to `$FFFF8880`
/// (row 17, column 0): columns 3, 15, 9, 9, 15.
const STRIP_COLUMNS: [i32; 5] = [3, 15, 9, 9, 15];
/// The strip's plane row (`$FFFF8880` is row 17) and size in cells
/// (`moveq #$10, d1; moveq #4, d2`, `ps4.asm:2111`).
const STRIP_ROW: i32 = 17;
const STRIP_WIDTH: i32 = 16;
const STRIP_HEIGHT: i32 = 4;
/// The icon words, four per icon in reading order (top-left, top-right,
/// bottom-left, bottom-right): `BattleTiles_AttackIcon` through
/// `BattleTiles_DefenseIcon` (`ps4.asm:11258`).
pub(super) const ICON_WORDS: [[u16; 4]; 10] = [
    [0xE167, 0xE168, 0xE175, 0xE176],
    [0xE15E, 0xE15F, 0xE16E, 0xE16F],
    [0xE160, 0xE161, 0xE170, 0xE171],
    [0xE162, 0xE962, 0xF162, 0xF962],
    [0xE163, 0xE164, 0xE172, 0xE173],
    [0xE165, 0xE166, 0xE174, 0xE974],
    [0xE169, 0xE16A, 0xE177, 0xE178],
    [0xE16B, 0xE16C, 0xE179, 0xE17A],
    [0xE16D, 0xE96D, 0xE17B, 0xE97B],
    [0xF17B, 0xF97B, 0xE17B, 0xE97B],
];
/// The icon cells sit at window-local columns 1, 4, 7, 10, 13 of row 1
/// (`$82, $88, $8E, $94, $9A` from the window base, `ps4.asm:2126-2159`).
const ICON_LOCAL_COLUMN: [i32; 5] = [1, 4, 7, 10, 13];
/// The cursor sprite: x steps 24 pixels per icon from the first icon cell
/// (`Battle_CharCommand`, `ps4.asm:2200`, multiplies the index by 24 and adds
/// `Battle_ComdCursorInitPos`'s `$A8, $108, $D8, $D8, $108`, less the 128 of
/// the sprite plane and the mapping's -8), at y `$120 - 4 - 128 = 156`
/// (`$2E(a4)`, `ps4.asm:70478`, and the mapping's `$FC`).
const CURSOR_Y: i32 = 156;
const CURSOR_STEP: i32 = 24;
/// The cursor's two mapping frames toggle every six ticks
/// (`Mappings_ComdCursor`, `ps4.asm:70494`): tile `$17C`, then `$180`.
const CURSOR_TICKS_PER_FRAME: u32 = 6;
const CURSOR_FRAME_TILES: [u16; 2] = [CURSOR_FIRST_TILE, CURSOR_FIRST_TILE + 4];

/// First plane column of a list window for each acting slot: `loc_19D0`,
/// `loc_1D5E` and `loc_2154` all add `$08, $20, $14, $14, $20` bytes to
/// `$FFFF8600` (row 12): columns 4, 16, 10, 10, 16.
const LIST_COLUMNS: [i32; 5] = [4, 16, 10, 10, 16];
const LIST_ROW: i32 = 12;
const LIST_HEIGHT: i32 = 9;
/// The row of the first entry and the pitch between entries: `$82, $182,
/// $282, $382` from the window base (`ps4.asm:2515-2527`).
const ENTRY_FIRST_ROW: i32 = 13;
const ENTRY_PITCH: i32 = 2;
/// The page arrow: window art tile `$6E6`, drawn into a corner cell.
const ARROW_TILE: u16 = 0x6E6;

/// The cursor tiles of a red-cursor row (`$6E8` red, `$6E7` blue).
const RED_CURSOR: u16 = 0x6E8;
const BLUE_CURSOR: u16 = 0x6E7;

/// CRAM line of an entry's text: line 3 when it can be chosen, line 2 when the
/// routine ORs `$C000` into its tile words (`ps4.asm:2543`).
const fn entry_line(enabled: bool) -> usize {
    if enabled { 3 } else { 2 }
}

/// Draws the strip.
pub(super) fn draw_strip(
    chrome: &BattleChrome,
    tiles: &mut CommandTiles,
    menu: &CommandMenuView,
    quads: &mut Vec<Quad>,
) {
    let Some(strip) = menu.strip.as_ref() else {
        return;
    };
    let column = STRIP_COLUMNS[usize::from(strip.slot).min(4)];
    let cell = chrome.cell;
    if let Some(frame) = chrome.frame(WindowRect {
        x: column as f32 * cell,
        y: STRIP_ROW as f32 * cell,
        w: STRIP_WIDTH as f32 * cell,
        h: STRIP_HEIGHT as f32 * cell,
    }) {
        quads.extend(frame);
    }
    for (icon, present) in strip.present.iter().enumerate() {
        if !present {
            continue;
        }
        let first = column + ICON_LOCAL_COLUMN[icon];
        for (index, word) in ICON_WORDS[icon + 1].iter().enumerate() {
            let at = tile_dest(
                first + (index as i32 % 2),
                STRIP_ROW + 1 + (index as i32 / 2),
            );
            if let Some(quad) = tiles.word(*word, at) {
                quads.push(quad);
            }
        }
    }
}

/// The cursor sprite, drawn over the planes. `None` while the window has not
/// had a frame yet: the object is created on the frame the window finishes
/// and shows from the next (`ps4.asm:70465`).
pub(super) fn strip_cursor(tiles: &mut CommandTiles, menu: &CommandMenuView) -> Vec<Quad> {
    let Some(strip) = menu.strip.as_ref() else {
        return Vec::new();
    };
    if strip.age == 0 {
        return Vec::new();
    }
    let column = STRIP_COLUMNS[usize::from(strip.slot).min(4)];
    let x = (column + 1) * 8 + CURSOR_STEP * i32::from(strip.cursor);
    // The sprite table the VDP reads is DMA'd from the buffer a frame after the
    // object writes it, so the frame on screen is the one the clock had a tick
    // ago (`docs/battle/BATTLE_COMMAND_UI.md`, "Cursor clock").
    let frame = ((strip.age - 1) / CURSOR_TICKS_PER_FRAME) as usize % 2;
    tiles.sprite_2x2(CURSOR_FRAME_TILES[frame], x, CURSOR_Y)
}

/// The window width for each list kind: `moveq #$C, d1` (technique,
/// `ps4.asm:2509`), `$F` (skill, `ps4.asm:2841`), `$E` (item, `ps4.asm:3205`).
fn list_width(window: MenuPage) -> i32 {
    match window {
        MenuPage::Skills => 16,
        MenuPage::Items => 14,
        _ => 12,
    }
}

/// Draws a technique, skill or item window page.
pub(super) fn draw_list(
    chrome: &BattleChrome,
    menu: &CommandMenuView,
    red: bool,
    quads: &mut Vec<Quad>,
) {
    let Some(list) = menu.list.as_ref() else {
        return;
    };
    let width = list_width(list.window);
    let column = LIST_COLUMNS[usize::from(list.slot).min(4)];
    let cell = chrome.cell;
    // The two page arrows replace the frame's corner cells: `$66E6` (left) in
    // the top-left corner of a window with an earlier page, `$6EE6`
    // (mirrored) in the top-right of one with a later page (`ps4.asm:2519`,
    // `2909`, `3179`; the Left and Right tests read the same words back,
    // `ps4.asm:2603-2608`).
    let mut excluded = Vec::new();
    if list.more_before {
        excluded.push((0, 0));
    }
    if list.more_after {
        excluded.push((width - 1, 0));
    }
    if let Some(frame) = chrome.frame_excluding(
        WindowRect {
            x: column as f32 * cell,
            y: LIST_ROW as f32 * cell,
            w: width as f32 * cell,
            h: LIST_HEIGHT as f32 * cell,
        },
        &excluded,
    ) {
        quads.extend(frame);
    }
    if list.more_before
        && let Some(quad) =
            chrome.window_word(ARROW_TILE, false, false, tile_dest(column, LIST_ROW))
    {
        quads.push(quad);
    }
    if list.more_after
        && let Some(quad) = chrome.window_word(
            ARROW_TILE,
            true,
            false,
            tile_dest(column + width - 1, LIST_ROW),
        )
    {
        quads.push(quad);
    }
    for row in 0..4usize {
        let y = ENTRY_FIRST_ROW + ENTRY_PITCH * row as i32;
        let pattern = if row == usize::from(list.cursor) && red {
            RED_CURSOR
        } else {
            BLUE_CURSOR
        };
        if let Some(quad) = chrome.window_word(pattern, false, false, tile_dest(column + 1, y)) {
            quads.push(quad);
        }
        let Some(entry) = list.entries.get(row) else {
            continue;
        };
        let line = entry_line(entry.enabled);
        quads.extend(chrome.text_on(
            line,
            &entry.name,
            WindowRect {
                x: (column + 3) as f32 * cell,
                y: y as f32 * cell,
                w: (width - 4) as f32 * cell,
                h: cell,
            },
        ));
        let Some(value) = entry.value else {
            continue;
        };
        // `loc_27DC7A` writes three cells: hundreds, tens, ones, with blanks
        // in front of the leading digit (`ps4.asm:311352`). The technique's
        // cost starts five cells after the name, the skill's uses eight
        // (`lea $A(a1)` / `lea $10(a1)`), and the skill window then
        // overwrites the hundreds cell with window tile `$6FF` (`ps4.asm:2845`).
        let first = column
            + 3
            + match list.window {
                MenuPage::Skills => 8,
                _ => 5,
            };
        let digits = [value / 100 % 10, value / 10 % 10, value % 10];
        for (offset, digit) in digits.into_iter().enumerate() {
            let at = tile_dest(first + offset as i32, y);
            let leading_blank = match offset {
                0 => value < 100,
                1 => value < 10,
                _ => false,
            };
            if list.window == MenuPage::Skills && offset == 0 {
                if let Some(quad) = chrome.window_word_on(line, 0x6FF, false, false, at) {
                    quads.push(quad);
                }
            } else if !leading_blank
                && let Some(quad) = chrome.font_word_on(line, 0x7DA + u16::from(digit), at)
            {
                quads.push(quad);
            }
        }
    }
}
