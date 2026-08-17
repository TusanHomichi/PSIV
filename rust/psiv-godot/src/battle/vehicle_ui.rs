//! Retail vehicle command-menu data and drawing.

use super::PartyPlacement;
use super::chrome::{BattleChrome, Quad, WindowRect};
use super::layout::tile_dest;

/// `VehicleAttackNames`, `ps4.asm:321193-321201`.
pub(super) const SKILL_NAMES: [&str; 8] = [
    "CLUSTER", "GRAVITN", "TH.GRID", "X-BURST", "NAPALM", "NOTHING", "N-SPHER", "NOTHING",
];

#[derive(Clone, Copy)]
pub(super) struct SkillSlot {
    pub(super) id: u8,
    pub(super) current: u8,
    pub(super) max: u8,
}

pub(super) fn slots_for(party: &PartyPlacement) -> Vec<SkillSlot> {
    party
        .skills
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, id)| *id != 0)
        .map(|(slot, id)| SkillSlot {
            id,
            current: party.skill_uses[slot],
            max: party.max_skill_uses[slot],
        })
        .collect()
}

pub(super) fn move_cursor(cursor: usize, count: usize, delta: isize) -> usize {
    if count == 0 {
        return 0;
    }
    let last = count - 1;
    if delta.is_negative() {
        cursor.saturating_sub(delta.unsigned_abs()).min(last)
    } else {
        cursor.saturating_add(delta as usize).min(last)
    }
}

pub(super) fn selected(slots: &[SkillSlot], cursor: usize) -> Option<u8> {
    slots
        .get(cursor)
        .filter(|slot| slot.current > 0)
        .map(|slot| slot.id)
}

/// Draws the retail 14x6 `Battle_VehOpenSkills` window. The cartridge lays
/// the available slots into two columns; current/max uses remain visible even
/// when the current count is zero, while the `$C000` word makes that entry
/// unselectable.
pub(super) fn draw(
    chrome: &BattleChrome,
    slots: &[SkillSlot],
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
