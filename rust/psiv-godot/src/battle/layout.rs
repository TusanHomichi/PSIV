//! Plane-A layout helpers shared by the battle node and oracle assertions.

use super::chrome::{BattleChrome, Quad, WindowRect};
use super::ui::{
    BATTLE_CELL_PIXELS, PartyStatus, STATUS_HP_Y, STATUS_NAME_Y, STATUS_PANE_START_CELLS,
    STATUS_RECT, STATUS_SEPARATOR_COLUMNS, STATUS_TP_Y,
};
use godot::prelude::*;

pub(super) fn tile_dest(x_cell: i32, y_cell: i32) -> Rect2 {
    Rect2::new(
        Vector2::new(
            x_cell as f32 * BATTLE_CELL_PIXELS as f32,
            y_cell as f32 * BATTLE_CELL_PIXELS as f32,
        ),
        Vector2::new(BATTLE_CELL_PIXELS as f32, BATTLE_CELL_PIXELS as f32),
    )
}

pub(super) fn append_status_quads(
    chrome: &BattleChrome,
    party_status: &[PartyStatus],
    quads: &mut Vec<Quad>,
) {
    let excluded: Vec<(i32, i32)> = STATUS_SEPARATOR_COLUMNS
        .iter()
        .flat_map(|column| {
            let local_column = *column - STATUS_RECT.x as i32 / BATTLE_CELL_PIXELS;
            (0..6).map(move |row| (local_column, row))
        })
        .collect();
    if let Some(frame) = chrome.frame_excluding(STATUS_RECT, &excluded) {
        quads.extend(frame);
    }

    for column in STATUS_SEPARATOR_COLUMNS {
        for row in 0..6 {
            let pattern = if row == 0 || row == 5 { 0x6f4 } else { 0x6f5 };
            if let Some(quad) =
                chrome.window_word(pattern, false, row == 5, tile_dest(column, 21 + row))
            {
                quads.push(quad);
            }
        }
    }

    for (pane, start) in STATUS_PANE_START_CELLS.iter().copied().enumerate() {
        if pane > 0
            && pane < 4
            && let Some(member) = party_status.get(pane - 1)
        {
            quads.extend(chrome.text(
                &member.name,
                WindowRect {
                    x: (start + 1) as f32 * BATTLE_CELL_PIXELS as f32,
                    y: STATUS_NAME_Y,
                    w: 32.0,
                    h: 8.0,
                },
            ));
            quads.extend(chrome.battle_number(
                &member.hp.to_string(),
                WindowRect {
                    x: (start + 5) as f32 * BATTLE_CELL_PIXELS as f32,
                    y: STATUS_HP_Y,
                    w: 24.0,
                    h: 8.0,
                },
            ));
            quads.extend(chrome.battle_number(
                &member.tp.to_string(),
                WindowRect {
                    x: (start + 5) as f32 * BATTLE_CELL_PIXELS as f32,
                    y: STATUS_TP_Y,
                    w: 24.0,
                    h: 8.0,
                },
            ));
        }

        // `oracle/layouts/battle_command_idle.json` tile runs rows 24/25:
        // HP is 0x6f8,0x6f9,0x7f3 and TP is 0x6fa,0x6f9,0x7f3 at pane+1.
        for (row, first) in [(24, 0x6f8), (25, 0x6fa)] {
            for (offset, pattern) in [(0, first), (1, 0x6f9)] {
                if let Some(quad) =
                    chrome.window_word(pattern, false, false, tile_dest(start + 1 + offset, row))
                {
                    quads.push(quad);
                }
            }
            if let Some(quad) = chrome.font_word(0x7f3, tile_dest(start + 3, row)) {
                quads.push(quad);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;
    use std::path::Path;

    fn oracle(name: &str) -> Option<Value> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../oracle/layouts")
            .join(name);
        if !path.is_file() {
            return None;
        }
        serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
    }

    fn integer(value: &Value, field: &str) -> i32 {
        value[field].as_i64().expect("oracle layout integer") as i32
    }

    fn chrome_rect<'a>(doc: &'a Value, kind: &str) -> &'a Value {
        doc["planes"]["plane_a"]["chrome_rectangles"]
            .as_array()
            .expect("oracle chrome rectangles")
            .iter()
            .find(|rect| rect["kind"] == kind)
            .unwrap_or_else(|| panic!("missing decoded {kind} rectangle"))
    }

    fn assert_rect(doc: &Value, kind: &str, rect: WindowRect) {
        let decoded = chrome_rect(doc, kind);
        assert_eq!(integer(decoded, "x_cell"), rect.x as i32 / 8);
        assert_eq!(integer(decoded, "y_cell"), rect.y as i32 / 8);
        assert_eq!(integer(decoded, "width_cells"), rect.w as i32 / 8);
        assert_eq!(integer(decoded, "height_cells"), rect.h as i32 / 8);
        let pixels = &decoded["pixel_rect"];
        assert_eq!(integer(pixels, "x"), rect.x as i32);
        assert_eq!(integer(pixels, "y"), rect.y as i32);
        assert_eq!(integer(pixels, "width"), rect.w as i32);
        assert_eq!(integer(pixels, "height"), rect.h as i32);
    }

    fn assert_text(doc: &Value, text: &str, cell_x: i32, cell_y: i32, pixel_x: i32, pixel_y: i32) {
        let run = doc["planes"]["plane_a"]["text_runs"]
            .as_array()
            .expect("oracle text runs")
            .iter()
            .find(|run| run["text"] == text)
            .unwrap_or_else(|| panic!("missing decoded text run {text:?}"));
        assert_eq!(integer(run, "cell_x"), cell_x);
        assert_eq!(integer(run, "cell_y"), cell_y);
        assert_eq!(integer(run, "pixel_x"), pixel_x);
        assert_eq!(integer(run, "pixel_y"), pixel_y);
    }

    fn body_row_cells(doc: &Value, y: i32) -> Vec<i32> {
        let mut cells = Vec::new();
        for run in doc["planes"]["plane_a"]["tile_runs"]
            .as_array()
            .expect("oracle tile runs")
        {
            if integer(run, "cell_y") != y || integer(run, "palette_line") != 1 {
                continue;
            }
            let start = integer(run, "cell_x");
            let length = integer(run, "length_cells");
            cells.extend(start..start + length);
        }
        cells
    }

    #[test]
    fn scene_rects_and_text_origins_equal_decoded_layouts() {
        let Some(idle) = oracle("battle_command_idle.json") else {
            return;
        };
        assert_rect(
            &idle,
            "enemy_name_window",
            super::super::ui::ENEMY_NAME_RECT,
        );
        assert_rect(&idle, "command_window", super::super::ui::COMMAND_RECT);
        assert_rect(&idle, "status_strip", STATUS_RECT);
        assert_text(&idle, "ZORAN BULT", 3, 2, 24, 16);
        assert_text(&idle, "COMD", 6, 6, 48, 48);
        assert_text(&idle, "MACR", 6, 8, 48, 64);
        assert_text(&idle, "RUN", 6, 10, 48, 80);
        assert_text(&idle, "Chaz", 10, 22, 80, 176);
        assert_text(&idle, "Alys", 17, 22, 136, 176);
        assert_text(&idle, "Hahn", 24, 22, 192, 176);

        // `oracle/layouts/battle_command_idle.json`: the two enemy body
        // tile-run blocks occupy x=11..16 and x=23..28 at rows 9..14. The
        // scene's 6x6 texture origins must therefore be (88,72) and
        // (184,72), not guessed formation columns.
        assert_eq!(
            body_row_cells(&idle, 9),
            (11..17).chain(23..29).collect::<Vec<_>>()
        );
        assert_eq!(super::super::ui::enemy_sprite_origin(17, 6, 6), (88, 72));
        assert_eq!(super::super::ui::enemy_sprite_origin(29, 6, 6), (184, 72));

        for name in [
            "battle_command_return.json",
            "battle_attack_effect.json",
            "battle_followup.json",
        ] {
            let Some(doc) = oracle(name) else {
                return;
            };
            assert_rect(&doc, "status_strip", STATUS_RECT);
            assert_text(&doc, "Chaz", 10, 22, 80, 176);
            assert_text(&doc, "Alys", 17, 22, 136, 176);
            assert_text(&doc, "Hahn", 24, 22, 192, 176);
        }

        let Some(victory) = oracle("battle_victory.json") else {
            return;
        };
        assert_rect(&victory, "victory_window", super::super::ui::VICTORY_RECT);
        assert_rect(&victory, "status_strip", STATUS_RECT);
        assert_text(&victory, "Victory!", 11, 17, 88, 136);
    }

    #[test]
    fn decoded_sprite_coordinates_are_the_scene_reference() {
        let expected: [(&str, &[(i32, i32)]); 4] = [
            ("battle_attack_effect.json", &[(-56, 80), (307, 109)]),
            (
                "battle_command_return.json",
                &[
                    (217, 81),
                    (241, 81),
                    (217, 97),
                    (214, 113),
                    (217, 97),
                    (214, 113),
                ],
            ),
            (
                "battle_followup.json",
                &[(90, 86), (116, 113), (113, 121), (116, 113), (113, 121)],
            ),
            ("battle_victory.json", &[(-56, 80), (307, 109)]),
        ];
        for (name, positions) in expected {
            let Some(doc) = oracle(name) else {
                return;
            };
            let actual: Vec<(i32, i32)> = doc["sprites"]["entries"]
                .as_array()
                .expect("oracle sprite entries")
                .iter()
                .filter(|entry| entry["visible"] == true)
                .map(|entry| (integer(entry, "screen_x"), integer(entry, "screen_y")))
                .collect();
            assert_eq!(actual.as_slice(), positions);
        }
    }
}
