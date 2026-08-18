//! Camp STATUS and the root summary's split-font numeric fields.

use psiv_runtime::CampCharacter;

use super::chrome::{CampChrome, Quad, STATUS_SLASH_PATTERN};
use super::layout::STATUS_TEXT;

// The labels' letter cells are window patterns; the trailing colon is the
// ordinary menu font's tile $6B4 (glyph index 51) and must render through
// the font sheet — pushing $6B4 through the window-word decode picks the
// wrong art (the camp receipt shows the two-dot colon at that cell).
const HP_LABEL_PATTERNS: [u16; 2] = [0x6F8, 0x6F9];
const TP_LABEL_PATTERNS: [u16; 2] = [0x6FA, 0x6F9];

/// The retail status window emits `LV` with a blank cell before the numeric
/// run. The colon/string representation is semantic only; drawing it as
/// ordinary glyphs overwrites the summary window's right border.
pub(super) fn draw_level(chrome: &CampChrome, quads: &mut Vec<Quad>, level: u16, cell: (i32, i32)) {
    draw_text(chrome, quads, "LV", cell);
    // Level uses the ordinary menu-font decimal run ($69B onward). The
    // second decimal run is reserved for the HP/TP values in the summary
    // window and would render `1` as the wrong glyph at the receipt cell.
    draw_text(chrome, quads, &level.to_string(), (cell.0 + 3, cell.1));
}

pub(super) fn draw_status_pair(
    chrome: &CampChrome,
    quads: &mut Vec<Quad>,
    label: &str,
    current: u16,
    maximum: u16,
    cell: (i32, i32),
) {
    let label_patterns = status_label_patterns(label);
    if let Some(patterns) = label_patterns {
        for (column, pattern) in patterns.into_iter().enumerate() {
            if let Some(quad) = chrome.window_word(pattern, (cell.0 + column as i32, cell.1)) {
                quads.push(quad);
            }
        }
        quads.extend(chrome.text(":", (cell.0 + 2, cell.1)));
        if let Some(quad) = chrome.window_word(0x680, (cell.0 + 3, cell.1)) {
            quads.push(quad);
        }
    } else {
        draw_text(chrome, quads, label, cell);
    }
    quads.extend(chrome.number(&current.to_string(), (cell.0 + 4, cell.1)));
    if let Some(quad) = chrome.window_word(STATUS_SLASH_PATTERN, (cell.0 + 6, cell.1)) {
        quads.push(quad);
    }
    // Retail leaves the window-font blank cell immediately after the slash:
    // `25/ 25`, not the compact `25/25` form.
    quads.extend(chrome.number(&maximum.to_string(), (cell.0 + 8, cell.1)));
}

fn status_label_patterns(label: &str) -> Option<[u16; 2]> {
    match label {
        "HP: " => Some(HP_LABEL_PATTERNS),
        "TP: " => Some(TP_LABEL_PATTERNS),
        _ => None,
    }
}

pub(super) fn draw_status_text(
    chrome: &CampChrome,
    quads: &mut Vec<Quad>,
    character: &CampCharacter,
    money: u32,
) {
    let age = character
        .age
        .map_or_else(|| "--".to_owned(), |age| age.to_string());
    let lines = [
        (character.name.clone(), STATUS_TEXT[0].cell),
        (character.profession.clone(), STATUS_TEXT[1].cell),
        (format!("AGE : {age}"), STATUS_TEXT[3].cell),
        (
            format!("STRNGTH: {}", character.strength),
            STATUS_TEXT[6].cell,
        ),
        (
            format!("MENTAL : {}", character.mental),
            STATUS_TEXT[7].cell,
        ),
        (
            format!("AGILITY: {}", character.agility),
            STATUS_TEXT[8].cell,
        ),
        (
            format!("DEXTRTY: {}", character.dexterity),
            STATUS_TEXT[9].cell,
        ),
        (
            format!("ATK POW: {}", character.attack_power),
            STATUS_TEXT[10].cell,
        ),
        (
            format!("DFS POW: {}", character.defense_power),
            STATUS_TEXT[11].cell,
        ),
        (character.equipment[2].clone(), STATUS_TEXT[12].cell),
        (character.equipment[0].clone(), STATUS_TEXT[13].cell),
        (character.equipment[1].clone(), STATUS_TEXT[14].cell),
        (character.equipment[3].clone(), STATUS_TEXT[15].cell),
        (
            format!("EX: {}", character.experience),
            STATUS_TEXT[16].cell,
        ),
        (
            format!(
                "NX: {}",
                character
                    .next_level_experience
                    .map_or_else(|| "--".to_owned(), |value| value.to_string())
            ),
            STATUS_TEXT[17].cell,
        ),
        (format!("{money} MST"), STATUS_TEXT[18].cell),
    ];
    for (text, cell) in lines {
        draw_text(chrome, quads, &text, cell);
    }
    draw_level(chrome, quads, character.level, STATUS_TEXT[2].cell);
    draw_status_pair(
        chrome,
        quads,
        "HP: ",
        character.current_hp,
        character.max_hp,
        STATUS_TEXT[4].cell,
    );
    draw_status_pair(
        chrome,
        quads,
        "TP: ",
        character.current_tp,
        character.max_tp,
        STATUS_TEXT[5].cell,
    );
}

fn draw_text(chrome: &CampChrome, quads: &mut Vec<Quad>, text: &str, cell: (i32, i32)) {
    quads.extend(chrome.text(text, cell));
}

#[cfg(test)]
mod tests {
    use super::{HP_LABEL_PATTERNS, TP_LABEL_PATTERNS, status_label_patterns};

    #[test]
    fn root_receipt_status_colon_uses_the_window_pattern() {
        assert_eq!(status_label_patterns("HP: "), Some(HP_LABEL_PATTERNS));
        assert_eq!(status_label_patterns("TP: "), Some(TP_LABEL_PATTERNS));
        assert_eq!(HP_LABEL_PATTERNS[1], 0x6F9);
        assert_eq!(TP_LABEL_PATTERNS[1], 0x6F9);
    }
}
