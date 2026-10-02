//! Retail vehicle command-menu data and drawing.

use psiv_runtime::SkillSlotView;

use super::chrome::{BattleChrome, Quad, WindowRect};
use super::layout::tile_dest;

/// `VehicleAttackNames`, `ps4.asm:321193-321201`.
///
/// The name table is presentation: the runtime counts the record's slots
/// (`SkillSlotView { id, current, max }`) and this module prints the id the
/// cartridge's own table names.
pub(super) const SKILL_NAMES: [&str; 8] = [
    "CLUSTER", "GRAVITN", "TH.GRID", "X-BURST", "NAPALM", "NOTHING", "N-SPHER", "NOTHING",
];

/// Draws the retail 14x6 `Battle_VehOpenSkills` window. The cartridge lays
/// the available slots into two columns; current/max uses remain visible even
/// when the current count is zero, while the `$C000` word makes that entry
/// unselectable.
pub(super) fn draw(
    chrome: &BattleChrome,
    slots: &[SkillSlotView],
    cursor: usize,
    quads: &mut Vec<Quad>,
) {
    const RECT: WindowRect = WindowRect {
        x: 24.0,
        y: 40.0,
        w: 112.0,
        h: 48.0,
    };
    let Some(frame) = chrome.frame(RECT) else {
        return;
    };
    quads.extend(frame);
    for (index, slot) in slots.iter().copied().enumerate() {
        let column = index % 2;
        let row = index / 2;
        let x = RECT.x + 16.0 + column as f32 * 56.0;
        let y = RECT.y + 8.0 + row as f32 * 16.0;
        quads.extend(chrome.text(
            SKILL_NAMES[usize::from(slot.id.saturating_sub(1))],
            WindowRect {
                x,
                y,
                w: 48.0,
                h: 8.0,
            },
        ));
        quads.extend(chrome.text(
            &format!("{}/{}", slot.current, slot.max),
            WindowRect {
                x: x + 40.0,
                y,
                w: 16.0,
                h: 8.0,
            },
        ));
        if index == cursor {
            let pattern = if slot.current > 0 { 0x6e8 } else { 0x6e7 };
            if let Some(quad) = chrome.window_word(
                pattern,
                false,
                false,
                tile_dest(3 + (column as i32 * 7), 5 + row as i32 * 2),
            ) {
                quads.push(quad);
            }
        }
    }
}
