//! Automated verification of worked JSON examples in `docs/MODDING.md` (#787).
//!
//! Extracts every `json` fenced block and the documented schema-violation
//! example directly from `docs/MODDING.md`, validates them unconditionally
//! against synthetic schema fixtures (so hosted CI without a ROM catches drift)
//! and validates them end-to-end through `PackOverlay::load` against `runtime-pack`
//! whenever the local pack is present.

use super::*;
use crate::battle::EnemiesFile;
use crate::dialogue::TreeFile;
use crate::{BattleFiles, GameData};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_GUIDE_ID: AtomicUsize = AtomicUsize::new(1);

fn guide_temp_dir(label: &str) -> PathBuf {
    let n = NEXT_GUIDE_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "psiv-modding-guide-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn modding_md_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/MODDING.md")
}

fn real_pack() -> Option<PathBuf> {
    if let Some(env_pack) = std::env::var_os("PSIV_RUNTIME_PACK") {
        let pack = PathBuf::from(env_pack);
        if pack.join("manifest.json").is_file() {
            return Some(pack);
        }
    }
    let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
    pack.join("manifest.json").is_file().then_some(pack)
}

#[derive(Debug)]
struct GuideJsonBlock {
    heading: String,
    context: String,
    body: String,
    line: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum GuideTarget {
    Manifest,
    Patch(&'static str),
}

fn extract_json_blocks(markdown: &str) -> Vec<GuideJsonBlock> {
    let mut blocks = Vec::new();
    let mut heading = String::new();
    let mut context = String::new();
    let mut in_fence = false;
    let mut is_json = false;
    let mut start_line = 0usize;
    let mut body = String::new();

    for (idx, line) in markdown.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        if !in_fence {
            if let Some(rest) = trimmed.strip_prefix("```") {
                in_fence = true;
                let lang = rest.split_whitespace().next().unwrap_or("");
                is_json = lang.eq_ignore_ascii_case("json");
                start_line = line_no;
                body.clear();
            } else if trimmed.starts_with('#') {
                heading = trimmed.to_owned();
                context.clear();
            } else {
                context.push_str(line);
                context.push('\n');
            }
        } else if trimmed.starts_with("```") {
            if is_json {
                blocks.push(GuideJsonBlock {
                    heading: heading.clone(),
                    context: context.clone(),
                    body: body.clone(),
                    line: start_line,
                });
            }
            in_fence = false;
            is_json = false;
        } else if is_json {
            body.push_str(line);
            body.push('\n');
        }
    }
    assert!(!in_fence, "unclosed code fence in docs/MODDING.md");
    blocks
}

fn classify_block(block: &GuideJsonBlock) -> GuideTarget {
    if block.heading.contains("mod.json") {
        GuideTarget::Manifest
    } else if block.context.contains("patches/battle/enemies.json") {
        GuideTarget::Patch("battle/enemies.json")
    } else if block.context.contains("patches/battle/formations.json") {
        GuideTarget::Patch("battle/formations.json")
    } else if block.context.contains("dialogue/trees.json") {
        GuideTarget::Patch("dialogue/trees.json")
    } else if block.context.contains("patches/maps/010_Piata.json") {
        GuideTarget::Patch("maps/010_Piata.json")
    } else {
        panic!(
            "unmapped ```json block in docs/MODDING.md at line {} under `{}`; \
             add validation for it in rust/psiv-data/src/mods/guide_tests.rs",
            block.line, block.heading
        );
    }
}

/// Extracts the bad HP value (`70000`) documented under `## When an edit does not fit`.
fn extract_bad_hp_example(markdown: &str) -> u64 {
    let section = markdown
        .split("## When an edit does not fit")
        .nth(1)
        .expect("docs/MODDING.md must have `## When an edit does not fit`");
    let marker = "\"hp\": ";
    let rest = section
        .split(marker)
        .nth(1)
        .expect("`## When an edit does not fit` must document an invalid `\"hp\": <n>` value");
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let value: u64 = digits
        .parse()
        .expect("invalid `\"hp\": <n>` integer in docs/MODDING.md");
    assert!(
        value > u64::from(u16::MAX),
        "bad HP example in docs/MODDING.md must exceed u16::MAX, got {value}"
    );
    value
}

fn synthetic_enemies_json() -> Value {
    let mut properties = BTreeMap::new();
    for slot in [
        "fire",
        "ice",
        "electric",
        "water",
        "gravity",
        "biological",
        "psychic",
        "light",
        "energy",
        "destroy",
        "holy",
        "dark",
        "physical",
        "special",
    ] {
        properties.insert(slot.to_owned(), json!({"value": 2}));
    }
    json!({
        "format_version": PACK_FORMAT_VERSION,
        "count": 1,
        "properties": properties.keys().cloned().collect::<Vec<_>>(),
        "enemies": [{
            "id": 10,
            "symbol": "SyntheticMob",
            "display_name": "SYNTHETIC",
            "hp": 25,
            "stats": {
                "strength": 10,
                "mental": 10,
                "agility": 10,
                "dexterity": 10,
                "attack": 12,
                "defense": 4,
                "magic_defense": 4
            },
            "attack": {
                "element": {"id": 0, "name": "none"},
                "status_effect": {"id": 0, "name": "none"}
            },
            "properties": properties,
            "ai": {
                "condition_ids": [0, 0, 0, 0],
                "conditional_ability_ids": [0, 0, 0, 0],
                "regular_ability_ids": [0, 0, 0, 0, 0, 0, 0, 0]
            },
            "rewards": {"experience": 10, "meseta": 10}
        }]
    })
}

fn synthetic_trees_json() -> Value {
    json!({
        "format_version": PACK_FORMAT_VERSION,
        "tree_count": 1,
        "entry_count": 1,
        "trees": [{
            "tree": 34,
            "label": "SyntheticTree34",
            "is_talk_tree": false,
            "portrait_operand_bytes": 1,
            "entry_count": 1,
            "entries": [{
                "id": 0,
                "text": "Professor!\nThank goodness you're safe!",
                "segments": [
                    {"code": "0xF4", "ctrl": "portrait", "operands": [2], "id": 2},
                    {"text": "Professor!"},
                    {"code": "0xFC", "ctrl": "newline", "operands": []},
                    {"text": "Thank goodness you're safe!"}
                ],
                "pages": [{
                    "lines": ["Professor!", "Thank goodness you're safe!"],
                    "end": "end"
                }]
            }]
        }],
        "window": {
            "chars_per_line": 32,
            "lines_per_window": 2,
            "glyph_width": 8,
            "glyph_height": 16,
            "rect": {"x": 8, "y": 160, "width": 256, "height": 32},
            "scroll_arrow": {
                "screen_x": 288,
                "screen_y": 192,
                "png": "dialogue/scroll_arrow.png",
                "width": 16,
                "height": 8
            }
        }
    })
}

fn synthetic_map_json() -> Value {
    json!({
        "format_version": PACK_FORMAT_VERSION,
        "id": 16,
        "symbol": "Piata",
        "png": "maps/010_Piata.png",
        "png_over": null,
        "dimensions": {
            "width_chunks": 1,
            "height_chunks": 1,
            "width_cells": 2,
            "height_cells": 2,
            "width_pixels": 32,
            "height_pixels": 32,
            "cell_pixels": 16
        },
        "collision": {
            "plane": "bg",
            "plane_byte": 1,
            "width_cells": 2,
            "height_cells": 2,
            "rows": [[0, 0], [0, 0]]
        },
        "music": {"id": 132, "symbol": "MotabiaTown", "changes_music": true},
        "flags": {
            "poison": 0,
            "random_battles": 0,
            "town_teleport": 1,
            "dungeon_teleport_index": 0
        },
        "dialogue_tree": 2,
        "events": [0],
        "warps": [],
        "interaction_areas": [],
        "npcs": [],
        "treasure_chests": []
    })
}

fn synthetic_formations_json() -> Value {
    json!({
        "format_version": PACK_FORMAT_VERSION,
        "formation_count": 1,
        "boss_formation_count": 0,
        "formations": [{
            "id": 0,
            "ambush_chance": 0,
            "run_chance": 0,
            "can_run": true,
            "drop_rate": 0,
            "drop_item": null,
            "count": 1,
            "count_matches_entries": true,
            "enemies": [{"slot": 1, "enemy_id": 10, "position": 14}]
        }],
        "boss_formations": [],
        "inline_formations": []
    })
}

#[test]
fn modding_guide_examples_parse_and_validate_against_schema() {
    let markdown = fs::read_to_string(modding_md_path()).expect("read docs/MODDING.md");
    let blocks = extract_json_blocks(&markdown);
    assert!(
        blocks.len() >= 5,
        "expected manifest + enemy + dialogue + map collision + additive formation JSON examples in docs/MODDING.md, found {}",
        blocks.len()
    );

    let mut manifest_text: Option<String> = None;
    let mut enemy_patch: Option<Value> = None;
    let mut dialogue_patch: Option<Value> = None;
    let mut map_patch: Option<Value> = None;
    let mut formation_patch: Option<Value> = None;

    for block in &blocks {
        let parsed: Value = serde_json::from_str(&block.body).unwrap_or_else(|err| {
            panic!(
                "invalid JSON in docs/MODDING.md at line {}: {err}",
                block.line
            )
        });
        match classify_block(block) {
            GuideTarget::Manifest => {
                let manifest: ModManifest =
                    serde_json::from_value(parsed).expect("mod.json example must deserialize");
                manifest
                    .validate(Path::new(MOD_MANIFEST_FILE))
                    .expect("mod.json example must validate");
                manifest_text = Some(block.body.clone());
            }
            GuideTarget::Patch("battle/enemies.json") => {
                let mut base = synthetic_enemies_json();
                apply_json_patch(&mut base, &parsed, "battle/enemies.json")
                    .expect("MODDING.md enemy patch must apply");
                let typed: EnemiesFile =
                    serde_json::from_value(base).expect("patched EnemiesFile must satisfy schema");
                assert_eq!(typed.enemies[0].hp, 99);
                enemy_patch = Some(parsed);
            }
            GuideTarget::Patch("battle/formations.json") => {
                let mut base = synthetic_formations_json();
                apply_json_patch(&mut base, &parsed, "battle/formations.json")
                    .expect("MODDING.md formation patch must apply");
                let typed: crate::battle::FormationsFile = serde_json::from_value(base.clone())
                    .expect("patched FormationsFile must satisfy schema");
                assert_eq!(typed.formation_count, 2);
                assert_eq!(typed.formations.len(), 2);
                assert_eq!(typed.formations[1].id, Some(504));
                let diff = diff_json(
                    &synthetic_formations_json(),
                    &base,
                    "battle/formations.json",
                )
                .expect("strict diff_json must accept appended formation")
                .expect("non-empty formation diff");
                assert_eq!(diff, parsed);
                formation_patch = Some(parsed);
            }
            GuideTarget::Patch("dialogue/trees.json") => {
                let mut base = synthetic_trees_json();
                apply_json_patch(&mut base, &parsed, "dialogue/trees.json")
                    .expect("MODDING.md dialogue patch must apply");
                let typed: TreeFile =
                    serde_json::from_value(base).expect("patched TreeFile must satisfy schema");
                let entry = typed.trees[0].entry(0).unwrap();
                assert!(
                    entry.text.contains("Glad to see you in one piece!"),
                    "{}",
                    entry.text
                );
                for page in &entry.pages {
                    assert!(page.lines.len() <= 2, "window fits at most 2 lines");
                    for line in &page.lines {
                        assert!(
                            line.chars().count() <= 32,
                            "dialogue line exceeds 32-char window: {line:?}"
                        );
                    }
                }
                dialogue_patch = Some(parsed);
            }
            GuideTarget::Patch("maps/010_Piata.json") => {
                let mut base = synthetic_map_json();
                apply_json_patch(&mut base, &parsed, "maps/010_Piata.json")
                    .expect("MODDING.md map collision patch must apply");
                let typed: crate::MapRecord = serde_json::from_value(base.clone())
                    .expect("patched MapRecord must satisfy schema");
                assert_eq!(
                    typed.collision.grid.at(1, 1),
                    Some(crate::CollisionType::Solid)
                );
                // Round-trip through strict diff_json must reproduce the sparse cell patch.
                let diff = diff_json(&synthetic_map_json(), &base, "maps/010_Piata.json")
                    .expect("strict diff_json must accept single-cell collision edit")
                    .expect("non-empty collision diff");
                assert_eq!(diff, parsed);
                map_patch = Some(parsed);
            }
            GuideTarget::Patch(other) => panic!("unexpected patch target {other}"),
        }
    }

    let manifest_text = manifest_text.expect("docs/MODDING.md missing mod.json example");
    let enemy_patch = enemy_patch.expect("docs/MODDING.md missing battle/enemies.json example");
    let dialogue_patch =
        dialogue_patch.expect("docs/MODDING.md missing dialogue/trees.json example");
    let map_patch = map_patch.expect("docs/MODDING.md missing maps/010_Piata.json example");
    let formation_patch =
        formation_patch.expect("docs/MODDING.md missing battle/formations.json example");

    // Negative control (always runs, even without runtime-pack): the bad HP value
    // documented under `## When an edit does not fit` must fail EnemiesFile parsing.
    let bad_hp = extract_bad_hp_example(&markdown);
    let mut bad_enemies_patch = enemy_patch.clone();
    bad_enemies_patch["enemies"]["10"]["hp"] = json!(bad_hp);
    let mut bad_base = synthetic_enemies_json();
    apply_json_patch(&mut bad_base, &bad_enemies_patch, "battle/enemies.json").unwrap();
    let bad_err = serde_json::from_value::<EnemiesFile>(bad_base)
        .expect_err("documented out-of-range HP must fail EnemiesFile deserialization");
    let bad_msg = bad_err.to_string();
    assert!(bad_msg.contains(&bad_hp.to_string()), "{bad_msg}");
    assert!(bad_msg.contains("u16"), "{bad_msg}");

    // Full end-to-end verification against the real ROM-built runtime-pack when present.
    let Some(base_pack) = real_pack() else {
        eprintln!("runtime-pack absent; synthetic MODDING.md verification passed");
        return;
    };

    let scratch = guide_temp_dir("real-pack");
    let mod_dir = scratch.join("mods/guide-mod");
    fs::create_dir_all(mod_dir.join("patches/battle")).unwrap();
    fs::create_dir_all(mod_dir.join("patches/dialogue")).unwrap();
    fs::create_dir_all(mod_dir.join("patches/maps")).unwrap();
    fs::write(mod_dir.join(MOD_MANIFEST_FILE), &manifest_text).unwrap();
    fs::write(
        mod_dir.join("patches/battle/enemies.json"),
        serde_json::to_string_pretty(&enemy_patch).unwrap(),
    )
    .unwrap();
    fs::write(
        mod_dir.join("patches/battle/formations.json"),
        serde_json::to_string_pretty(&formation_patch).unwrap(),
    )
    .unwrap();
    fs::write(
        mod_dir.join("patches/dialogue/trees.json"),
        serde_json::to_string_pretty(&dialogue_patch).unwrap(),
    )
    .unwrap();
    fs::write(
        mod_dir.join("patches/maps/010_Piata.json"),
        serde_json::to_string_pretty(&map_patch).unwrap(),
    )
    .unwrap();

    let loaded = load_mod(&mod_dir).expect("docs/MODDING.md mod.json must load cleanly");
    let overlay = PackOverlay::load(&base_pack, &[loaded])
        .expect("docs/MODDING.md examples must pass full PackOverlay::load + schema validation");
    assert_eq!(
        overlay.patched_files(),
        &[
            "battle/enemies.json",
            "battle/formations.json",
            "dialogue/trees.json",
            "maps/010_Piata.json"
        ]
    );

    let battle = BattleFiles::load_overlay(&overlay).unwrap();
    let zoran = battle.enemies.enemies.iter().find(|e| e.id == 10).unwrap();
    assert_eq!(zoran.hp, 99);
    assert!(
        battle
            .formations
            .formations
            .iter()
            .any(|f| f.id == Some(504))
    );
    let game = GameData::load_overlay(&overlay).unwrap();
    let piata = game.map(crate::MapId(0x010)).unwrap();
    assert_eq!(
        piata.collision.grid.at(1, 1),
        Some(crate::CollisionType::Solid)
    );
    let dialogue = game.dialogue().unwrap();
    let entry = dialogue
        .trees
        .trees
        .iter()
        .find(|t| t.tree == 34)
        .unwrap()
        .entry(0)
        .unwrap();
    assert!(entry.text.contains("Glad to see you in one piece!"));

    // Strict default diff_packs must round-trip all four guide examples cleanly.
    let edited_copy = scratch.join("edited-guide");
    tests::materialize_overlay_for_diff_test(&overlay, &edited_copy);
    let exported = scratch.join("exported-guide");
    let exported_summary = diff_packs(
        &base_pack,
        &edited_copy,
        &exported,
        &ModManifest::new("exported-guide", "Exported Guide", "1.0.0"),
    )
    .expect("strict diff_packs must round-trip all MODDING.md examples");
    assert_eq!(
        exported_summary.patches,
        vec![
            "battle/enemies.json",
            "battle/formations.json",
            "dialogue/trees.json",
            "maps/010_Piata.json"
        ]
    );
    assert!(exported_summary.derived_items.is_empty());
    let exported_map_patch: Value = serde_json::from_str(
        &fs::read_to_string(exported.join("patches/maps/010_Piata.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(exported_map_patch, map_patch);

    // Negative control through full `PackOverlay::load` naming mod, file, and place.
    let bad_dir = scratch.join("mods/tough-zoran");
    init_mod_dir(
        &bad_dir,
        &ModManifest::new("tough-zoran", "Tough Zoran", "1.0.0"),
    )
    .unwrap();
    fs::create_dir_all(bad_dir.join("patches/battle")).unwrap();
    fs::write(
        bad_dir.join("patches/battle/enemies.json"),
        serde_json::to_string(&bad_enemies_patch).unwrap(),
    )
    .unwrap();
    let err = PackOverlay::load(&base_pack, &[load_mod(&bad_dir).unwrap()])
        .expect_err("documented invalid HP example must be rejected by PackOverlay::load");
    assert_eq!(err.mod_id, "tough-zoran");
    assert_eq!(err.file, "battle/enemies.json");
    assert!(err.detail.contains(&bad_hp.to_string()), "{err}");
    assert!(err.detail.contains("u16"), "{err}");

    let _ = fs::remove_dir_all(&scratch);
}
