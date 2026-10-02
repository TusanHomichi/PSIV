//! The corpus: every retail entry, against the extractor's own pagination.
//!
//! `psiv_tools.dialogue_pack.paginate` is an independent transcription of
//! `RunText_CharacterLoop`, from the ROM, and its pages are what the pack
//! carries. If the flow and the extractor ever disagree, one of them is wrong
//! about the cartridge.

use super::tests::{pack_dir, pages};
use psiv_data::{DialogueSet, PageEnd};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// The dialogue decoder cannot see the VDP window plane or infer a portrait
/// from a page that never emits `$F4`, so this proof is opt-in and consumes a
/// small sidecar made from the read-only oracle captures. The normal suite
/// stays useful without a local oracle checkout; an oracle run makes the
/// assertion strict.
fn dialogue_oracle() -> Option<Value> {
    let path = PathBuf::from(std::env::var_os("PSIV_DIALOGUE_LAYOUT_ORACLE")?);
    if !path.is_file() {
        eprintln!("skipping dialogue oracle: {} is absent", path.display());
        return None;
    }
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
    let document: Value = serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("could not parse {}: {error}", path.display()));
    if document["kind"] != "psiv_dialogue_layout_oracle" {
        panic!("{} is not a dialogue layout oracle sidecar", path.display());
    }
    Some(document)
}

fn oracle_rect(document: &Value, name: &str) -> [i32; 4] {
    let rect = &document[name];
    ["x", "y", "width", "height"].map(|field| {
        rect[field]
            .as_i64()
            .unwrap_or_else(|| panic!("oracle {name}.{field} is not an integer")) as i32
    })
}

#[test]
fn dialogue_window_matches_decoded_tape03_layout() {
    let Some(oracle) = dialogue_oracle() else {
        return;
    };
    assert_eq!(oracle["captures"]["window"]["frame"], 7400);
    assert_eq!(oracle["captures"]["window"]["self_check_passed"], true);
    assert_eq!(oracle["captures"]["arrow"]["frame"], 7371);
    assert_eq!(oracle["captures"]["arrow"]["self_check_passed"], true);

    let dir = pack_dir();
    let set = DialogueSet::load(&dir).expect("the dialogue pack loads");
    let text = set.window.text_window.rect;
    assert_eq!(
        [text.x, text.y, text.width, text.height],
        oracle_rect(&oracle, "window_rect")
    );
    let portrait = set.window.portrait_window.rect;
    assert_eq!(
        [portrait.x, portrait.y, portrait.width, portrait.height],
        oracle_rect(&oracle, "portrait_rect")
    );

    let border = set.window.geometry.border_cells as i32 * set.window.geometry.cell_pixels as i32;
    assert_eq!(
        [text.x + border, text.y + border],
        [
            oracle["text_origin"]["x"]
                .as_i64()
                .expect("oracle text origin x") as i32,
            oracle["text_origin"]["y"]
                .as_i64()
                .expect("oracle text origin y") as i32,
        ]
    );

    let arrow = &set.trees.window.scroll_arrow;
    assert_eq!(
        [
            arrow.screen_x,
            arrow.screen_y,
            arrow.width as i32,
            arrow.height as i32
        ],
        [
            oracle["arrow"]["screen_x"]
                .as_i64()
                .expect("oracle arrow x") as i32,
            oracle["arrow"]["screen_y"]
                .as_i64()
                .expect("oracle arrow y") as i32,
            oracle["arrow"]["width"]
                .as_i64()
                .expect("oracle arrow width") as i32,
            oracle["arrow"]["height"]
                .as_i64()
                .expect("oracle arrow height") as i32,
        ]
    );
    assert_eq!(oracle["arrow"]["pattern"], 0x7F6);
    assert_eq!(oracle["arrow"]["palette_line"], 2);

    assert_eq!(oracle["portrait_present"], false);
    assert!(!oracle["gaps"].as_array().expect("oracle gaps").is_empty());
    eprintln!(
        "dialogue oracle gap: tape 03 frame 7400 has no portrait `$F4`; portrait position is decoded from the retail window group, not visible art"
    );
}

#[test]
fn every_retail_entry_pages_exactly_as_the_extractor_says() {
    let dir = pack_dir();
    if !dir.join("dialogue").join("trees.json").is_file() {
        eprintln!(
            "skipping: no dialogue pack at {}. Build one with \
             `python -m psiv_tools pack <rom> runtime-pack/`.",
            dir.display()
        );
        return;
    }
    let set = DialogueSet::load(&dir).expect("the dialogue pack loads");

    let mut compared = 0usize;
    let mut shown = 0usize;
    for tree in &set.trees.trees {
        for entry in &tree.entries {
            let expected: Vec<(Vec<String>, PageEnd)> = entry
                .pages
                .iter()
                .map(|page| (page.lines.clone(), page.end))
                .collect();
            assert_eq!(
                pages(entry),
                expected,
                "{} entry {} paginates differently",
                tree.label,
                entry.id
            );
            compared += 1;
            shown += expected.len();
        }
    }
    assert_eq!(compared, 2736);
    // 6,038 windows across the corpus: 4,258 $FD waits, 1,538 message ends,
    // 126 $F7 closes, 89 full windows and 27 choices.
    assert_eq!(shown, 6038);
}

#[test]
fn the_corpus_never_needs_a_third_line_or_a_wrapped_word() {
    let dir = pack_dir();
    if !dir.join("dialogue").join("trees.json").is_file() {
        return;
    }
    let set = DialogueSet::load(&dir).expect("the dialogue pack loads");
    for tree in &set.trees.trees {
        for entry in &tree.entries {
            for (lines, _) in pages(entry) {
                assert!(lines.len() <= 2, "{} entry {}", tree.label, entry.id);
                for line in lines {
                    assert!(
                        line.chars().count() <= 32,
                        "{} entry {}",
                        tree.label,
                        entry.id
                    );
                }
            }
        }
    }
}
