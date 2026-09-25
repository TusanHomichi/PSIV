//! Decoded Genesis-frame geometry for the field camp menu.
//!
//! The cartridge stores window groups one cell short in each dimension; the
//! constants below are the rendered outer rectangles, in the 40×28 visible
//! cell frame. The tests are deliberately oracle-gated so a source checkout
//! without the private decode still builds, while a checkout with it cannot
//! silently drift.

#[cfg(test)]
use serde_json::Value;

/// The retail camp cell size.
pub(super) const CELL: f32 = 8.0;

/// A rendered outer rectangle in camp cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CellRect {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) w: i32,
    pub(super) h: i32,
}

impl CellRect {
    pub(super) const fn new(x: i32, y: i32, w: i32, h: i32) -> CellRect {
        CellRect { x, y, w, h }
    }
}

/// A text run origin and its decoded retail string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TextAnchor {
    pub(super) text: &'static str,
    pub(super) cell: (i32, i32),
}

/// The root ITEM/TECH/SKILL/EQUIP/STATE/MUMBL/MACRO window.
// docs/camp/CAMP_MENU_LAYOUT.md § Root; oracle/layouts/camp_menu_camera_decode.json camp_root.menu_root
pub(super) const ROOT_MENU: CellRect = CellRect::new(4, 2, 9, 15);
/// The one-member summary retained beside the root options.
// docs/camp/CAMP_MENU_LAYOUT.md § Root; oracle/layouts/camp_menu_camera_decode.json camp_root.character_summary
pub(super) const CHARACTER_SUMMARY: CellRect = CellRect::new(26, 1, 12, 6);
/// The root/state meseta strip.
// docs/camp/CAMP_MENU_LAYOUT.md § Root and § STATE chooser; oracle/... camp_root.meseta
pub(super) const MESETA: CellRect = CellRect::new(3, 23, 13, 3);
/// The decoded empty-inventory ITEM message.
// docs/camp/CAMP_MENU_LAYOUT.md § ITEM: empty inventory; oracle/... camp_item_empty_inventory.item_message
pub(super) const ITEM_MESSAGE: CellRect = CellRect::new(7, 21, 26, 5);
/// The decoded STATE child chooser.
// docs/camp/CAMP_MENU_LAYOUT.md § STATE chooser; oracle/... camp_state_chooser.state_options
#[cfg(test)]
pub(super) const STATE_OPTIONS: CellRect = CellRect::new(2, 5, 10, 5);
/// Modern STATE chooser geometry with the save action added below the two
/// retail rows. `STATE_OPTIONS` remains the oracle-pinned retail rectangle.
pub(super) const STATE_SAVE_OPTIONS: CellRect = CellRect::new(2, 5, 10, 7);
/// The disk-save slot chooser. Retail's system SAVE window is wider and sits
/// on the system-menu path; this is the explicit camp-owned seam.
pub(super) const SAVE_SLOTS_OPTIONS: CellRect = CellRect::new(7, 5, 26, 9);
/// The STATUS portrait tile block.
// docs/camp/CAMP_MENU_LAYOUT.md § STATUS; oracle/... camp_status.portrait
pub(super) const STATUS_PORTRAIT: CellRect = CellRect::new(3, 2, 10, 10);
/// The STATUS character-info window.
// docs/camp/CAMP_MENU_LAYOUT.md § STATUS; oracle/... camp_status.character_info
pub(super) const STATUS_INFO: CellRect = CellRect::new(13, 2, 12, 12);
/// The STATUS combat-statistics window.
// docs/camp/CAMP_MENU_LAYOUT.md § STATUS; oracle/... camp_status.combat_stats
pub(super) const STATUS_STATS: CellRect = CellRect::new(25, 2, 13, 13);
/// The STATUS read-only equipment window.
// docs/camp/CAMP_MENU_LAYOUT.md § STATUS; oracle/... camp_status.equipment
pub(super) const STATUS_EQUIPMENT: CellRect = CellRect::new(3, 14, 12, 9);
/// The STATUS experience/next-level window.
// docs/camp/CAMP_MENU_LAYOUT.md § STATUS; oracle/... camp_status.exp_next
pub(super) const STATUS_EXP: CellRect = CellRect::new(25, 21, 12, 5);

/// The retail EQUIP character chooser groups `$93..$96`, decoded from
/// `WinGroup_Menu` (`ps4.asm:140623-140633`). The width and origin stay fixed;
/// the height grows by two cells per additional party member.
pub(super) const fn equip_character_list(party_len: usize) -> CellRect {
    let height = match party_len {
        0 | 1 => 6,
        2 => 8,
        3 => 10,
        _ => 12,
    };
    CellRect::new(2, 13, 9, height)
}

/// The retail EQUIP stats group `$97`: `[10,08,02,02]` plus one-cell
/// dimensions, so outer `(2,2,17,9)`.
// reference/ps4disasm/ps4.asm:140635-140636
pub(super) const EQUIP_STATS: CellRect = CellRect::new(2, 2, 17, 9);
/// The retail equipped-item group `$C0`: `[0E,09,03,0C]` plus dimensions.
// reference/ps4disasm/ps4.asm:140758-140759
pub(super) const EQUIPPED_ITEMS: CellRect = CellRect::new(3, 12, 15, 10);
/// The first retail EQUIP item-list page `$98`: `[0D,11,14,02]` plus
/// dimensions. Later pages only reduce the visible height or move right.
// reference/ps4disasm/ps4.asm:140638-140639
pub(super) const EQUIP_ITEM_LIST: CellRect = CellRect::new(20, 2, 14, 18);
/// EQUIP's result/message group `$C1`, shared with the decoded ITEM message.
// reference/ps4disasm/ps4.asm:140761-140762
pub(super) const EQUIP_MESSAGE: CellRect = ITEM_MESSAGE;

/// Root menu strings and their decoded origins.
// docs/camp/CAMP_MENU_LAYOUT.md § Root; oracle/... camp_root.text_runs
pub(super) const ROOT_TEXT: &[TextAnchor] = &[
    TextAnchor {
        text: "Chaz",
        cell: (27, 2),
    },
    TextAnchor {
        text: "LV  : 1",
        cell: (33, 2),
    },
    TextAnchor {
        text: "HP: 25/25",
        cell: (27, 4),
    },
    TextAnchor {
        text: "TP: 10/10",
        cell: (27, 5),
    },
    TextAnchor {
        text: "ITEM",
        cell: (7, 3),
    },
    TextAnchor {
        text: "TECH",
        cell: (7, 5),
    },
    TextAnchor {
        text: "SKILL",
        cell: (7, 7),
    },
    TextAnchor {
        text: "EQUIP",
        cell: (7, 9),
    },
    TextAnchor {
        text: "STATE",
        cell: (7, 11),
    },
    TextAnchor {
        text: "MUMBL",
        cell: (7, 13),
    },
    TextAnchor {
        text: "MACRO",
        cell: (7, 15),
    },
    TextAnchor {
        text: "500 MST",
        cell: (8, 24),
    },
];

/// Root cursor: a one-cell red plane/SAT cursor.
// docs/camp/CAMP_MENU_LAYOUT.md § Root cursor; oracle/layouts/camp_root.json sprites.entries[0]
pub(super) const ROOT_CURSOR_CELL: (i32, i32) = (5, 3);
/// The decoded selected cursor word.
pub(super) const SELECTED_CURSOR_PATTERN: u16 = 0x6E8;
/// Hollow child cursor in the STATE chooser.
// docs/camp/CAMP_MENU_LAYOUT.md § STATE chooser cursor; oracle/... camp_state_chooser.cursor.child_selection
pub(super) const STATE_CURSOR_CELL: (i32, i32) = (3, 6);
/// The decoded hollow child cursor word.
pub(super) const CHILD_CURSOR_PATTERN: u16 = 0x6E7;

/// Text in the empty ITEM child.
// docs/camp/CAMP_MENU_LAYOUT.md § ITEM: empty inventory; oracle/... camp_item_empty_inventory.text_runs
pub(super) const ITEM_EMPTY_TEXT: TextAnchor = TextAnchor {
    text: "Can't have any items!",
    cell: (8, 22),
};

/// Text in the STATE chooser child.
// docs/camp/CAMP_MENU_LAYOUT.md § STATE chooser; oracle/... camp_state_chooser.text_runs
pub(super) const STATE_TEXT: &[TextAnchor] = &[
    TextAnchor {
        text: "STATUS",
        cell: (5, 6),
    },
    TextAnchor {
        text: "ORDER",
        cell: (5, 8),
    },
];

/// The added modern save row; the two retail STATE strings stay above it.
pub(super) const STATE_SAVE_TEXT: TextAnchor = TextAnchor {
    text: "SAVE",
    cell: (5, 10),
};

/// Prompt and visible three-slot labels for the camp save flow.
pub(super) const SAVE_SLOT_TEXT: &[TextAnchor] = &[
    TextAnchor {
        text: "SAVE IN NUMBER- ?",
        cell: (9, 6),
    },
    TextAnchor {
        text: "1",
        cell: (11, 8),
    },
    TextAnchor {
        text: "2",
        cell: (11, 10),
    },
    TextAnchor {
        text: "3",
        cell: (11, 12),
    },
];

/// Text in the decoded one-member STATUS screen.
// docs/camp/CAMP_MENU_LAYOUT.md § STATUS; oracle/... camp_status.text_runs
pub(super) const STATUS_TEXT: &[TextAnchor] = &[
    TextAnchor {
        text: "Chaz",
        cell: (15, 3),
    },
    TextAnchor {
        text: "HUNTER",
        cell: (15, 5),
    },
    TextAnchor {
        text: "LV  : 1",
        cell: (15, 7),
    },
    TextAnchor {
        text: "AGE : 16",
        cell: (15, 9),
    },
    TextAnchor {
        text: "HP: 25/25",
        cell: (14, 11),
    },
    TextAnchor {
        text: "TP: 10/10",
        cell: (14, 12),
    },
    TextAnchor {
        text: "STRNGTH: 8",
        cell: (26, 3),
    },
    TextAnchor {
        text: "MENTAL : 6",
        cell: (26, 5),
    },
    TextAnchor {
        text: "AGILITY: 7",
        cell: (26, 7),
    },
    TextAnchor {
        text: "DEXTRTY: 5",
        cell: (26, 9),
    },
    TextAnchor {
        text: "ATK POW: 18",
        cell: (26, 11),
    },
    TextAnchor {
        text: "DFS POW: 10",
        cell: (26, 13),
    },
    TextAnchor {
        text: "LTHR-HELM",
        cell: (4, 15),
    },
    TextAnchor {
        text: "HUNT-KNIFE",
        cell: (4, 17),
    },
    TextAnchor {
        text: "HUNT-KNIFE",
        cell: (4, 19),
    },
    TextAnchor {
        text: "LTHR-CLOTH",
        cell: (4, 21),
    },
    TextAnchor {
        text: "EX: 0",
        cell: (26, 22),
    },
    TextAnchor {
        text: "NX: 21",
        cell: (26, 24),
    },
    TextAnchor {
        text: "500 MST",
        cell: (8, 24),
    },
    TextAnchor {
        text: "MUMBL",
        cell: (7, 13),
    },
];

#[cfg(test)]
fn oracle() -> Option<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/layouts/camp_menu_camera_decode.json");
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
fn screen<'a>(doc: &'a Value, id: &str) -> &'a Value {
    doc["screens"]
        .as_array()
        .expect("camp camera decode screens")
        .iter()
        .find(|screen| screen["id"] == id)
        .unwrap_or_else(|| panic!("camp camera decode screen {id}"))
}

#[cfg(test)]
fn assert_rect(screen: &Value, id: &str, expected: CellRect) {
    let rect = screen["windows"]
        .as_array()
        .expect("decoded windows")
        .iter()
        .find(|window| window["id"] == id)
        .unwrap_or_else(|| panic!("decoded window {id}"));
    let cells: Vec<i32> = rect["rect_cells"]
        .as_array()
        .expect("decoded rect cells")
        .iter()
        .map(|value| value.as_i64().expect("integer rect cell") as i32)
        .collect();
    assert_eq!(cells, [expected.x, expected.y, expected.w, expected.h]);
    let pixels: Vec<i32> = rect["rect_pixels"]
        .as_array()
        .expect("decoded rect pixels")
        .iter()
        .map(|value| value.as_i64().expect("integer rect pixel") as i32)
        .collect();
    assert_eq!(
        pixels,
        [
            expected.x * 8,
            expected.y * 8,
            expected.w * 8,
            expected.h * 8
        ]
    );
}

#[cfg(test)]
fn assert_text(screen: &Value, expected: TextAnchor) {
    let found = screen["text_runs"]
        .as_array()
        .expect("decoded text runs")
        .iter()
        .find(|run| {
            run["text"] == expected.text
                && run["cell"] == serde_json::json!([expected.cell.0, expected.cell.1])
        })
        .unwrap_or_else(|| panic!("decoded text {:?}", expected.text));
    let cell: Vec<i32> = found["cell"]
        .as_array()
        .expect("decoded text cell")
        .iter()
        .map(|value| value.as_i64().expect("integer text cell") as i32)
        .collect();
    assert_eq!(cell, [expected.cell.0, expected.cell.1]);
    let pixel: Vec<i32> = found["pixel"]
        .as_array()
        .expect("decoded text pixel")
        .iter()
        .map(|value| value.as_i64().expect("integer text pixel") as i32)
        .collect();
    assert_eq!(pixel, [expected.cell.0 * 8, expected.cell.1 * 8]);
}

#[cfg(test)]
fn assert_cell(value: &Value, expected: (i32, i32)) {
    let cell: Vec<i32> = value
        .as_array()
        .expect("decoded cursor cell")
        .iter()
        .map(|value| value.as_i64().expect("integer cursor cell") as i32)
        .collect();
    assert_eq!(cell, [expected.0, expected.1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_rects_text_and_cursor_match_decode() {
        let Some(doc) = oracle() else { return };
        let root = screen(&doc, "camp_root");
        assert_rect(root, "menu_root", ROOT_MENU);
        assert_rect(root, "character_summary", CHARACTER_SUMMARY);
        assert_rect(root, "meseta", MESETA);
        for text in ROOT_TEXT {
            assert_text(root, *text);
        }
        assert_cell(&root["cursor"]["cell"], ROOT_CURSOR_CELL);
        assert_eq!(
            root["cursor"]["form"].as_str(),
            Some("one-cell red SAT cursor; pattern 0x6E8, palette line 2, priority")
        );
    }

    #[test]
    fn item_empty_rects_text_and_retained_cursor_match_decode() {
        let Some(doc) = oracle() else { return };
        let item = screen(&doc, "camp_item_empty_inventory");
        assert_rect(item, "menu_root", ROOT_MENU);
        assert_rect(item, "character_summary", CHARACTER_SUMMARY);
        assert_rect(item, "item_message", ITEM_MESSAGE);
        for text in ROOT_TEXT.iter().skip(4).take(7) {
            assert_text(item, *text);
        }
        assert_text(item, ITEM_EMPTY_TEXT);
        assert_cell(&item["cursor"]["cell"], ROOT_CURSOR_CELL);
    }

    #[test]
    fn state_chooser_rects_text_and_cursors_match_decode() {
        let Some(doc) = oracle() else { return };
        let state = screen(&doc, "camp_state_chooser");
        assert_rect(state, "menu_root", ROOT_MENU);
        assert_rect(state, "character_summary", CHARACTER_SUMMARY);
        assert_rect(state, "state_options", STATE_OPTIONS);
        assert_rect(state, "meseta", MESETA);
        for text in STATE_TEXT {
            assert_text(state, *text);
        }
        assert_text(
            state,
            TextAnchor {
                text: "STATE",
                cell: (7, 11),
            },
        );
        assert_text(
            state,
            TextAnchor {
                text: "500 MST",
                cell: (8, 24),
            },
        );
        assert_cell(&state["cursor"]["root_selection"]["cell"], (5, 11));
        assert_cell(
            &state["cursor"]["child_selection"]["cell"],
            STATE_CURSOR_CELL,
        );
        assert!(
            state["cursor"]["root_selection"]["form"]
                .as_str()
                .is_some_and(|form| form.contains("0x6E8"))
        );
        assert!(
            state["cursor"]["child_selection"]["form"]
                .as_str()
                .is_some_and(|form| form.contains("0x6E7"))
        );
    }

    #[test]
    fn status_rects_and_text_match_decode() {
        let Some(doc) = oracle() else { return };
        let status = screen(&doc, "camp_status");
        assert_rect(status, "portrait", STATUS_PORTRAIT);
        assert_rect(status, "character_info", STATUS_INFO);
        assert_rect(status, "combat_stats", STATUS_STATS);
        assert_rect(status, "equipment", STATUS_EQUIPMENT);
        assert_rect(status, "exp_next", STATUS_EXP);
        assert_rect(status, "meseta", MESETA);
        for text in STATUS_TEXT {
            assert_text(status, *text);
        }
        assert!(
            status["cursor"]["form"]
                .as_str()
                .is_some_and(|form| form.contains("none visible"))
        );
    }
}
