//! The dialogue pack: the real thing, and one fixture per way it can be wrong.
//!
//! Same gating as `runtime_pack.rs`: the pack is Sega-derived and never
//! committed, so a fresh clone skips the corpus tests. The fixture tests are
//! synthetic and always run -- every validation rule this crate enforces has to
//! be provably reachable, or it is decoration.
//!
//! The fixtures are written as text rather than built with `serde_json`:
//! `serde_json` is a dependency of the crate, not a dev-dependency, so an
//! integration test cannot name it, and a broken pack is a *string* anyway.

use psiv_data::{
    ActionKind, CHARS_PER_LINE, Ctrl, DataError, DialogueSet, FlagScope, LINES_PER_WINDOW,
    PORTRAIT_HIDE, PageEnd, Role, Segment,
};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// The real pack
// ---------------------------------------------------------------------------

fn pack_dir() -> PathBuf {
    match std::env::var_os("PSIV_RUNTIME_PACK") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("runtime-pack"),
    }
}

/// The pack, or `None` with a note on stderr when the dialogue slice is absent.
fn require_dialogue() -> Option<DialogueSet> {
    let dir = pack_dir();
    if !dir.join("dialogue").join("trees.json").is_file() {
        eprintln!(
            "skipping: no dialogue pack at {}. Build one with \
             `python -m psiv_tools pack <rom> runtime-pack/`, or set PSIV_RUNTIME_PACK.",
            dir.display()
        );
        return None;
    }
    Some(DialogueSet::load(&dir).expect("the real dialogue pack loads and validates"))
}

#[test]
fn the_real_pack_loads_and_validates() {
    let Some(set) = require_dialogue() else {
        return;
    };

    // 43 Kosinski trees from 0x1DF600 to 0x1FE655 (docs/source-notes/formats.md).
    assert_eq!(set.trees.trees.len(), 43);
    assert_eq!(set.trees.tree_count, 43);
    let entries: usize = set.trees.trees.iter().map(|t| t.entries.len()).sum();
    assert_eq!(entries, set.trees.entry_count as usize);

    // Two talk trees take a second $F4 operand; the other 41 take one.
    let talk = set.trees.trees.iter().filter(|t| t.is_talk_tree).count();
    assert_eq!(talk, 2);
    for tree in &set.trees.trees {
        let expected = if tree.is_talk_tree { 2 } else { 1 };
        assert_eq!(tree.portrait_operand_bytes, expected, "{}", tree.label);
    }
}

#[test]
fn the_first_entry_of_the_first_tree_reads_as_the_cartridge_shows_it() {
    let Some(set) = require_dialogue() else {
        return;
    };
    let entry = set.entry(1, 0).expect("tree 1, entry 0");

    // Three $FA checks, then the text the not-set branch shows.
    let checks = entry
        .segments
        .iter()
        .filter(|s| matches!(s, Segment::Control(Ctrl::FlagCheck { .. })))
        .count();
    assert_eq!(checks, 3);
    assert!(matches!(
        entry.segments[0],
        Segment::Control(Ctrl::FlagCheck {
            flag: 218,
            scope: FlagScope::EventFlag,
            then_entry: 3,
            ..
        })
    ));

    assert_eq!(entry.pages.len(), 2);
    assert_eq!(entry.pages[0].lines, ["Are you a hunter?"]);
    assert_eq!(entry.pages[0].end, PageEnd::Wait);
    assert_eq!(
        entry.pages[1].lines,
        ["Are you here to exterminate", "the monsters?"]
    );
    assert_eq!(entry.pages[1].end, PageEnd::End);
}

#[test]
fn every_page_fits_the_window_and_every_glyph_exists() {
    let Some(set) = require_dialogue() else {
        return;
    };
    let mut lines = 0usize;
    let mut exactly_full = 0usize;
    for tree in &set.trees.trees {
        for entry in &tree.entries {
            for page in &entry.pages {
                assert!(page.lines.len() <= LINES_PER_WINDOW);
                for line in &page.lines {
                    let width = line.chars().count();
                    assert!(width <= CHARS_PER_LINE, "{}: {line:?}", tree.label);
                    lines += 1;
                    exactly_full += usize::from(width == CHARS_PER_LINE);
                }
            }
            for segment in &entry.segments {
                if let Segment::Text(text) = segment {
                    for ch in text.chars() {
                        assert!(set.glyph(ch).is_some(), "no glyph for {ch:?}");
                    }
                }
            }
        }
    }
    // A census of the pack as it stands: no page line exceeds 32 characters
    // and 335 are exactly 32. If that number moves, the decoder changed shape.
    // (`psiv_tools/dialogue_pack.py`'s module docstring says 291, which is
    // neither the page-line count nor the 338 lines of decoded `text` -- a
    // stale number in that lane's prose, not a defect in this data.)
    assert_eq!(exactly_full, 335);
    assert!(lines > 9_000, "only {lines} lines?");
}

#[test]
fn nothing_in_the_corpus_needs_word_wrapping() {
    let Some(set) = require_dialogue() else {
        return;
    };
    // Every line break in retail text is explicit ($FC) or the column counter
    // wrapping at exactly 32. A renderer that wraps on word boundaries would
    // therefore be inventing layout, so this is the evidence for not having
    // one: no page line is over the limit anywhere in the corpus.
    let over = set
        .trees
        .trees
        .iter()
        .flat_map(|t| &t.entries)
        .flat_map(|e| &e.pages)
        .flat_map(|p| &p.lines)
        .filter(|line| line.chars().count() > CHARS_PER_LINE)
        .count();
    assert_eq!(over, 0);
}

#[test]
fn every_portrait_code_resolves_and_every_portrait_has_art() {
    let Some(set) = require_dialogue() else {
        return;
    };
    let dir = pack_dir();
    let mut shown = 0usize;
    let mut hidden = 0usize;
    for tree in &set.trees.trees {
        for entry in &tree.entries {
            for segment in &entry.segments {
                let Segment::Control(Ctrl::Portrait { id, position, .. }) = segment else {
                    continue;
                };
                if *id == PORTRAIT_HIDE {
                    hidden += 1;
                    continue;
                }
                shown += 1;
                let portrait = set.portrait(*id).expect("portrait resolves");
                assert!(dir.join(&portrait.png).is_file(), "{}", portrait.png);
                // The talk slot only exists in talk trees.
                assert_eq!(position.is_some(), tree.is_talk_tree, "{}", tree.label);
            }
        }
    }
    assert!(hidden > 0 && shown > 0);
    assert_eq!(set.portraits.portraits.len(), 39);
    assert!(set.portrait(PORTRAIT_HIDE).is_none());
}

#[test]
fn the_window_describes_the_box_the_cartridge_draws() {
    let Some(set) = require_dialogue() else {
        return;
    };
    let text = &set.window.text_window;
    // 272x48 at (24, 160) in Genesis screen space: 34x6 cells, a one-cell
    // border around 32x4 cells of text.
    assert_eq!((text.rect.x, text.rect.y), (24, 160));
    assert_eq!((text.rect.width, text.rect.height), (272, 48));
    assert_eq!(text.chars_per_line as usize, CHARS_PER_LINE);
    assert_eq!(text.lines_per_window as usize, LINES_PER_WINDOW);
    assert_eq!((text.glyph_width, text.glyph_height), (8, 16));

    let portrait = &set.window.portrait_window;
    assert_eq!((portrait.rect.x, portrait.rect.y), (40, 104));
    assert_eq!((portrait.rect.width, portrait.rect.height), (48, 48));

    for role in Role::ALL {
        let tile = set.window.role(role).expect("role present");
        assert_eq!((tile.width, tile.height), (8, 8));
        assert!(tile.priority, "{role} draws over the field");
        assert_eq!(tile.x, tile.tile as i32 * 8, "{role} strip position");
    }
    // The frame is one corner and two edges plus their flips.
    let corner = set.window.role(Role::CornerTopLeft).unwrap();
    for (role, flip_h, flip_v) in [
        (Role::CornerTopRight, true, false),
        (Role::CornerBottomLeft, false, true),
        (Role::CornerBottomRight, true, true),
    ] {
        let tile = set.window.role(role).unwrap();
        assert_eq!(tile.tile, corner.tile, "{role}");
        assert_eq!((tile.flip_h, tile.flip_v), (flip_h, flip_v), "{role}");
    }
    assert_eq!(set.window.geometry.open_animation.step_cells, 2);
    assert_eq!(set.window.geometry.border_cells, 1);
    assert_eq!(set.window.geometry.cell_pixels, 8);
}

#[test]
fn the_font_is_the_strip_the_renderer_indexes() {
    let Some(set) = require_dialogue() else {
        return;
    };
    assert_eq!((set.font.glyph.width, set.font.glyph.height), (8, 16));
    assert_eq!(set.font.glyphs.len(), set.font.glyph.count as usize);
    // Cell N of the strip is font byte N, so x is byte * 8.
    for glyph in &set.font.glyphs {
        assert_eq!(glyph.x, i32::from(glyph.byte) * 8);
        assert_eq!(glyph.y, 0);
    }
    // The dialogue charset, not the window charset: 'a' is 27, not 57.
    assert_eq!(set.font.by_char[&'a'], 27);
    assert_eq!(set.font.by_char[&' '], 0);
    assert_eq!(set.glyph('A').expect("A").x, 8);
    // Font bytes with no character are declared, not silently missing.
    assert_eq!(set.font.unmapped_glyphs, [78, 79]);
}

#[test]
fn actions_are_named_not_numbered() {
    let Some(set) = require_dialogue() else {
        return;
    };
    let mut panels = 0usize;
    for tree in &set.trees.trees {
        for entry in &tree.entries {
            for segment in &entry.segments {
                if let Segment::Control(Ctrl::Action {
                    action, action_id, ..
                }) = segment
                {
                    // The name and the TextActionsOffs index agree.
                    let expected = match action {
                        ActionKind::LoadPanel => 0,
                        ActionKind::DestroyLastPanel => 1,
                        ActionKind::DestroyAllPanels => 2,
                        ActionKind::LoadSound => 3,
                        ActionKind::LoadSound2 => 4,
                        ActionKind::UpdatePalette => 6,
                        ActionKind::ZioEyesRed => 7,
                        ActionKind::PauseMusic => 8,
                        ActionKind::ResumeMusic => 9,
                        ActionKind::SabotageAlarmRedPalette => 10,
                        ActionKind::SetEventFlag => 11,
                        ActionKind::ElsydeonBroken => 12,
                    };
                    assert_eq!(*action_id, expected);
                    panels += usize::from(*action == ActionKind::LoadPanel);
                }
            }
        }
    }
    assert_eq!(panels, 165);
}

// ---------------------------------------------------------------------------
// Fixtures: one per rule
// ---------------------------------------------------------------------------

const FONT: &str = r#"{
  "format_version": 1,
  "png": "dialogue/font.png",
  "by_char": {" ": 0, "A": 1, "H": 8, "i": 35},
  "glyphs": [
    {"byte": 0, "char": " ", "x": 0, "y": 0, "width": 8, "height": 16, "blank": true},
    {"byte": 1, "char": "A", "x": 8, "y": 0, "width": 8, "height": 16, "blank": false},
    {"byte": 8, "char": "H", "x": 64, "y": 0, "width": 8, "height": 16, "blank": false},
    {"byte": 35, "char": "i", "x": 280, "y": 0, "width": 8, "height": 16, "blank": false}
  ],
  "glyph": {"count": 4, "width": 8, "height": 16},
  "unmapped_glyphs": [78, 79]
}"#;

const PORTRAITS: &str = r#"{
  "format_version": 1,
  "count": 1,
  "portraits": [
    {"id": 1, "symbol": "Chaz", "png": "dialogue/portraits/01_Chaz.png", "duplicate_of": null}
  ],
  "geometry": {
    "width": 48, "height": 48, "screen_x": 40, "screen_y": 104,
    "talk_slot_stride_tiles": 12
  }
}"#;

fn window_json() -> String {
    let role = |tile: u32, x: i32, h: bool, v: bool| {
        format!(
            r#"{{"tile": {tile}, "x": {x}, "y": 0, "width": 8, "height": 8,
                 "flip_h": {h}, "flip_v": {v}, "priority": true}}"#
        )
    };
    format!(
        r#"{{
  "format_version": 1,
  "png": "dialogue/window.png",
  "geometry": {{
    "border_cells": 1, "cell_pixels": 8, "shadow": false,
    "open_animation": {{"axis": "horizontal", "from": "center", "step_cells": 2,
                        "frames_per_step": 1}}
  }},
  "roles": {{
    "corner_top_left": {}, "corner_top_right": {},
    "corner_bottom_left": {}, "corner_bottom_right": {},
    "edge_top": {}, "edge_bottom": {},
    "edge_left": {}, "edge_right": {},
    "fill": {}
  }},
  "text_window": {{
    "window": 1,
    "rect": {{"x": 24, "y": 160, "width": 272, "height": 48}},
    "chars_per_line": 32, "lines_per_window": 2,
    "glyph_width": 8, "glyph_height": 16,
    "interior": {{"x_cell": 4, "y_cell": 21, "width_cells": 32, "height_cells": 4}}
  }},
  "portrait_window": {{
    "window": 2,
    "rect": {{"x": 40, "y": 104, "width": 48, "height": 48}}
  }},
  "windows": [],
  "palette": {{"colors": [[0, 36, 109], [255, 255, 255]], "fill_index": 14}}
}}"#,
        role(105, 840, false, false),
        role(105, 840, true, false),
        role(105, 840, false, true),
        role(105, 840, true, true),
        role(106, 848, false, false),
        role(106, 848, false, true),
        role(115, 920, false, false),
        role(115, 920, true, false),
        role(0, 0, false, false),
    )
}

/// One tree, two entries: a portrait plus a page, and an empty entry.
fn trees_json() -> String {
    r#"{
  "format_version": 1,
  "tree_count": 1,
  "entry_count": 2,
  "window": {
    "chars_per_line": 32, "lines_per_window": 2, "glyph_width": 8, "glyph_height": 16,
    "rect": {"x": 32, "y": 168, "width": 256, "height": 32},
    "scroll_arrow": {
      "screen_x": 264, "screen_y": 202,
      "png": "dialogue/scroll_arrow.png", "width": 16, "height": 8
    }
  },
  "trees": [
    {
      "tree": 1,
      "label": "DialogueTree1",
      "is_talk_tree": false,
      "portrait_operand_bytes": 1,
      "entry_count": 2,
      "entries": [
        {
          "id": 0,
          "text": "Hi",
          "segments": [
            {"code": "0xF4", "ctrl": "portrait", "operands": [1], "id": 1, "position": null},
            {"text": "Hi"},
            {"code": "0xFD", "ctrl": "wait", "operands": []},
            {"code": "0xFA", "ctrl": "flag_check", "operands": [11, 1], "flag": 11,
             "scope": "event_flag", "then_entry": 1}
          ],
          "pages": [{"lines": ["Hi"], "end": "wait"}]
        },
        {"id": 1, "text": "", "segments": [], "pages": []}
      ]
    }
  ]
}"#
    .to_owned()
}

/// Writes a fixture pack under the test's own temp dir and loads it.
fn load_fixture(
    name: &str,
    trees: &str,
    font: &str,
    window: &str,
    portraits: &str,
) -> Result<DialogueSet, DataError> {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let dir = root.join("dialogue");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(dir.join("portraits")).expect("fixture dir");
    std::fs::write(dir.join("trees.json"), trees).expect("trees");
    std::fs::write(dir.join("font.json"), font).expect("font");
    std::fs::write(dir.join("window.json"), window).expect("window");
    std::fs::write(dir.join("portraits.json"), portraits).expect("portraits");
    std::fs::write(dir.join("portraits").join("01_Chaz.png"), b"png").expect("portrait art");
    std::fs::write(dir.join("font.png"), b"png").expect("font art");
    std::fs::write(dir.join("window.png"), b"png").expect("window art");
    DialogueSet::load(&root)
}

/// The fixture with one thing changed, and the failure it has to produce.
fn rejects(name: &str, trees: String, fragment: &str) {
    let error = load_fixture(name, &trees, FONT, &window_json(), PORTRAITS)
        .expect_err("the fixture is broken and must be rejected");
    let message = error.to_string();
    assert!(
        message.contains(fragment),
        "expected {fragment:?} in: {message}"
    );
}

#[test]
fn the_fixture_itself_is_valid() {
    let set = load_fixture(
        "dialogue_ok",
        &trees_json(),
        FONT,
        &window_json(),
        PORTRAITS,
    )
    .expect("the base fixture loads");
    assert_eq!(set.entry(1, 0).expect("entry 0").pages.len(), 1);
    assert!(set.entry(1, 2).is_none());
    assert_eq!(set.entry(1, 1).expect("entry 1").segments.len(), 0);
    assert_eq!(set.portrait(1).expect("Chaz").symbol, "Chaz");
}

#[test]
fn invalid_scene_tree_addresses_stop_the_load() {
    rejects(
        "dialogue_bad_rom_offset",
        trees_json().replace(r#""tree": 1,"#, r#""tree": 1, "rom_offset": "unresolved","#),
        "invalid rom_offset",
    );
    let mut fixture: serde_json::Value = serde_json::from_str(&trees_json()).unwrap();
    fixture["trees"][0]["rom_offset"] = "0x1DF600".into();
    let mut second = fixture["trees"][0].clone();
    second["tree"] = 2.into();
    second["label"] = "DialogueTree2".into();
    fixture["trees"].as_array_mut().unwrap().push(second);
    fixture["tree_count"] = 2.into();
    fixture["entry_count"] = 4.into();
    rejects(
        "dialogue_duplicate_rom_offset",
        fixture.to_string(),
        "duplicate rom_offset",
    );
}

#[test]
fn an_unknown_control_name_stops_the_load() {
    rejects(
        "dialogue_unknown_ctrl",
        trees_json().replace(r#""ctrl": "wait""#, r#""ctrl": "shake_screen""#),
        "unknown variant `shake_screen`",
    );
}

#[test]
fn an_unknown_field_on_a_control_stops_the_load() {
    rejects(
        "dialogue_unknown_field",
        trees_json().replace(
            r#""ctrl": "wait", "operands": []"#,
            r#""ctrl": "wait", "operands": [], "speed": 3"#,
        ),
        "unknown field `speed`",
    );
}

#[test]
fn a_code_byte_that_disagrees_with_its_name_stops_the_load() {
    rejects(
        "dialogue_code_mismatch",
        trees_json().replace(
            r#""code": "0xFD", "ctrl": "wait""#,
            r#""code": "0xFB", "ctrl": "wait""#,
        ),
        "carries code 0xFB",
    );
}

#[test]
fn a_code_byte_that_is_not_hex_stops_the_load() {
    rejects(
        "dialogue_code_shape",
        trees_json().replace(r#""code": "0xFD""#, r#""code": "FD""#),
        "not 0x-prefixed",
    );
}

#[test]
fn a_text_segment_with_extra_keys_stops_the_load() {
    rejects(
        "dialogue_text_extra",
        trees_json().replace(r#"{"text": "Hi"}"#, r#"{"text": "Hi", "color": 3}"#),
        "carries nothing but `text`",
    );
}

#[test]
fn a_portrait_that_is_not_in_the_table_stops_the_load() {
    rejects(
        "dialogue_portrait_missing",
        trees_json().replace(r#""operands": [1], "id": 1"#, r#""operands": [7], "id": 7"#),
        "shows portrait 7",
    );
}

#[test]
fn a_character_with_no_glyph_stops_the_load() {
    rejects(
        "dialogue_glyph_missing",
        trees_json().replace(r#"{"text": "Hi"}"#, r#"{"text": "Hí"}"#),
        "font has no glyph for",
    );
}

#[test]
fn a_jump_past_the_end_of_the_tree_stops_the_load() {
    rejects(
        "dialogue_jump_range",
        trees_json().replace(r#""then_entry": 1"#, r#""then_entry": 9"#),
        "then_entry is 9 but the tree has 2 entries",
    );
}

#[test]
fn entry_ids_that_are_not_dense_stop_the_load() {
    rejects(
        "dialogue_sparse_ids",
        trees_json().replace(r#"{"id": 1, "text": ""#, r#"{"id": 4, "text": ""#),
        "must be id 1, not 4",
    );
}

#[test]
fn a_declared_count_that_does_not_match_stops_the_load() {
    rejects(
        "dialogue_bad_count",
        trees_json().replace(
            r#""entry_count": 2,
  "window""#,
            r#""entry_count": 5,
  "window""#,
        ),
        "entry_count is 5 but the trees hold 2 entries",
    );
}

#[test]
fn a_page_wider_than_the_window_stops_the_load() {
    let wide = "A".repeat(33);
    rejects(
        "dialogue_wide_page",
        trees_json().replace(r#""lines": ["Hi"]"#, &format!(r#""lines": ["{wide}"]"#)),
        "is 33 characters",
    );
}

#[test]
fn a_third_line_in_one_window_stops_the_load() {
    rejects(
        "dialogue_three_lines",
        trees_json().replace(r#""lines": ["Hi"]"#, r#""lines": ["Hi", "Hi", "Hi"]"#),
        "has 3 lines",
    );
}

#[test]
fn a_wrong_format_version_stops_the_load() {
    let error = load_fixture(
        "dialogue_version",
        &trees_json().replace(r#""format_version": 1"#, r#""format_version": 2"#),
        FONT,
        &window_json(),
        PORTRAITS,
    )
    .expect_err("version 2 is not this build's format");
    assert!(error.to_string().contains("format version 2"), "{error}");
}

#[test]
fn a_missing_window_role_stops_the_load() {
    let window = window_json().replace(r#""edge_left""#, r#""edge_middle""#);
    let error = load_fixture("dialogue_role", &trees_json(), FONT, &window, PORTRAITS)
        .expect_err("eight roles cannot draw a frame");
    assert!(error.to_string().contains("missing edge_left"), "{error}");
}

#[test]
fn a_text_window_that_does_not_fit_its_glyphs_stops_the_load() {
    let window = window_json().replace(r#""width": 272"#, r#""width": 264"#);
    let error = load_fixture("dialogue_geometry", &trees_json(), FONT, &window, PORTRAITS)
        .expect_err("264 is not 32 glyphs plus two border cells");
    assert!(error.to_string().contains("rect is 264x48"), "{error}");
}

#[test]
fn a_portrait_with_no_art_on_disk_stops_the_load() {
    let portraits = PORTRAITS.replace("01_Chaz.png", "01_Chazz.png");
    let error = load_fixture(
        "dialogue_art",
        &trees_json(),
        FONT,
        &window_json(),
        &portraits,
    )
    .expect_err("the art has to be in the pack");
    assert!(error.to_string().contains("01_Chazz.png"), "{error}");
}

#[test]
fn a_missing_file_names_itself() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("dialogue_absent");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("dir");
    let error = DialogueSet::load(&root).expect_err("there is no pack there");
    assert!(error.to_string().contains("trees.json"), "{error}");
    assert!(error.path().is_some());
}
