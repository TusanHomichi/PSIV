//! End-to-end tests for in-memory [`PackOverlay`] resolution (#791).
//!
//! Uses a complete synthetic pack fixture so every guarantee — no
//! `mods/_active_pack` staging directory, untouched `base_pack`, table patches,
//! dialogue shorthands, live-palette map PNG reconciliation, multi-mod asset
//! precedence, path-traversal/symlink rejection, and fail-closed schema
//! attribution — runs deterministically even without a ROM-built `runtime-pack`.

use super::*;
use crate::game_data::tests::fixtures::{DIGEST, MapJson, SKIPPED_MOTAVIA, manifest_text};
use crate::{MapId, PACK_FORMAT_VERSION};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

fn scratch_dir(label: &str) -> PathBuf {
    let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "psiv-overlay-test-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, bytes).unwrap();
}

/// Writes a minimal, fully valid synthetic pack that passes `GameData::load`
/// and `BattleFiles::load`, including dialogue, live-palette map redirects,
/// and enemy battle tables.
fn write_synthetic_pack(base: &Path) {
    let version = PACK_FORMAT_VERSION;
    let piata = MapJson::default();
    let academy = MapJson::academy();
    write_file(
        base,
        "manifest.json",
        manifest_text(version, &[&piata, &academy], SKIPPED_MOTAVIA, "").as_bytes(),
    );

    // Inject live-palette `map_update_images` redirects into Piata's map JSON.
    let mut piata_json: serde_json::Value = serde_json::from_str(&piata.text()).unwrap();
    piata_json["map_update_images"] = json!({
        "maps/010_Piata.png": "maps/010_Piata_indices.png",
        "maps/010_Piata_over.png": "maps/010_Piata_over_indices.png"
    });
    write_file(
        base,
        &piata.json_name(),
        serde_json::to_string_pretty(&piata_json)
            .unwrap()
            .as_bytes(),
    );
    write_file(base, &academy.json_name(), academy.text().as_bytes());
    write_file(base, "maps/010_Piata.png", b"base-piata-png");
    write_file(base, "maps/010_Piata_indices.png", b"base-piata-idx");
    write_file(base, "maps/010_Piata_over.png", b"base-piata-over");
    write_file(
        base,
        "maps/010_Piata_over_indices.png",
        b"base-piata-over-idx",
    );
    write_file(base, "maps/011_PiataAcademy_F1.png", b"base-academy-png");

    // Guild & fifth tables (required by GameData::load).
    let line = vec![[0u8, 0, 0]; 16];
    let rows = vec![[1u8; 8]; 8];
    let money = vec![1u8; 8];
    write_file(
        base,
        "guild/guild.json",
        json!({
            "format_version": 1, "kind": "guild", "rom_sha256": DIGEST,
            "window": {"id": 11, "width": 4, "height": 4, "x": 1, "y": 1},
            "names": ["a", "b", "c", "d", "e", "f", "g", "h"], "pending": "p",
            "rows": rows, "money": money,
            "font_line": line, "done_line": line, "closed_line": line
        })
        .to_string()
        .as_bytes(),
    );
    let fifth_row = json!({"x": 16, "y": 32, "character": 2, "first_line": 1, "picked_flag": 3});
    write_file(
        base,
        "fifth/fifth.json",
        json!({
            "format_version": 1, "kind": "fifth", "rom_sha256": DIGEST,
            "rows": [fifth_row, fifth_row, fifth_row, fifth_row, fifth_row],
            "records": [2, 4, 6, 8, 9]
        })
        .to_string()
        .as_bytes(),
    );

    // Fusion frames + credits + sprite index files.
    let frames: Vec<_> = (0..crate::FUSION_FRAMES)
        .map(|index| {
            let png = format!("battle/art/fusion/{index}.png");
            write_file(base, &png, b"fusion-png");
            json!({"index": index, "png": png, "origin": [-24, -32], "size": [48, 32]})
        })
        .collect();
    write_file(
        base,
        crate::FUSION_ART_PATH,
        json!({
            "format_version": version,
            "kind": "battle_fusion_art",
            "palette_enemy_id": 10,
            "frames": frames
        })
        .to_string()
        .as_bytes(),
    );
    write_file(
        base,
        crate::CREDITS_FILE,
        serde_json::to_string(&crate::presentation::credits::tests::fixture(DIGEST))
            .unwrap()
            .as_bytes(),
    );
    write_file(
        base,
        "sprites/party.json",
        format!(
            r#"{{"format_version":{version},"kind":"field_party","sheet_count":0,"sheets":[]}}"#
        )
        .as_bytes(),
    );
    write_file(
        base,
        "sprites/npcs.json",
        format!(
            r#"{{"format_version":{version},"kind":"field_npcs","sheet_count":0,"sheets":[]}}"#
        )
        .as_bytes(),
    );

    // Minimal dialogue set supporting replace + lines shorthands.
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz !'.0123456789,-?"
        .chars()
        .collect();
    let mut by_char = serde_json::Map::new();
    let mut glyphs = Vec::new();
    for (idx, ch) in chars.iter().enumerate() {
        let byte = (idx + 1) as u8;
        by_char.insert(ch.to_string(), json!(byte));
        glyphs.push(json!({
            "byte": byte,
            "char": ch.to_string(),
            "x": idx * 8,
            "y": 0,
            "width": 8,
            "height": 16,
            "blank": *ch == ' '
        }));
    }
    write_file(
        base,
        "dialogue/font.json",
        json!({
            "format_version": version,
            "png": "dialogue/font.png",
            "glyph": {"count": glyphs.len(), "width": 8, "height": 16},
            "glyphs": glyphs,
            "by_char": by_char,
            "unmapped_glyphs": []
        })
        .to_string()
        .as_bytes(),
    );
    write_file(base, "dialogue/font.png", b"base-font-png");
    let mut roles = serde_json::Map::new();
    for (tile, role) in crate::Role::ALL.iter().enumerate() {
        roles.insert(
            role.as_str().to_owned(),
            json!({
                "tile": tile,
                "x": tile * 8,
                "y": 0,
                "width": 8,
                "height": 8,
                "flip_h": false,
                "flip_v": false,
                "priority": true
            }),
        );
    }
    write_file(
        base,
        "dialogue/window.json",
        json!({
            "format_version": version,
            "png": "dialogue/window.png",
            "geometry": {
                "cell_pixels": 8,
                "border_cells": 1,
                "shadow": false,
                "open_animation": {
                    "axis": "horizontal",
                    "from": "center",
                    "step_cells": 2,
                    "frames_per_step": 1
                }
            },
            "text_window": {
                "window": 0,
                "rect": {"x": 24, "y": 160, "width": 272, "height": 48},
                "chars_per_line": 32,
                "lines_per_window": 2,
                "glyph_width": 8,
                "glyph_height": 16,
                "interior": {"x_cell": 4, "y_cell": 21, "width_cells": 32, "height_cells": 4}
            },
            "portrait_window": {
                "window": 1,
                "rect": {"x": 32, "y": 96, "width": 64, "height": 64}
            },
            "roles": roles,
            "windows": [],
            "menu_windows": [],
            "palette": {
                "colors": line,
                "fill_index": 14
            }
        })
        .to_string()
        .as_bytes(),
    );
    write_file(base, "dialogue/window.png", b"base-window-png");
    write_file(
        base,
        "dialogue/portraits.json",
        json!({
            "format_version": version,
            "count": 1,
            "portraits": [{"id": 2, "symbol": "Hahn", "png": "dialogue/portraits/02_Hahn.png"}],
            "geometry": {
                "width": 48,
                "height": 48,
                "screen_x": 40,
                "screen_y": 104,
                "talk_slot_stride_tiles": 12
            }
        })
        .to_string()
        .as_bytes(),
    );
    write_file(
        base,
        "dialogue/portraits/02_Hahn.png",
        b"base-hahn-portrait",
    );
    write_file(
        base,
        "dialogue/trees.json",
        json!({
            "format_version": version,
            "tree_count": 1,
            "entry_count": 2,
            "window": {
                "chars_per_line": 32,
                "lines_per_window": 2,
                "glyph_width": 8,
                "glyph_height": 16,
                "rect": {"x": 24, "y": 160, "width": 256, "height": 32},
                "scroll_arrow": {"png": "dialogue/arrow.png", "screen_x": 280, "screen_y": 192, "width": 16, "height": 8}
            },
            "trees": [{
                "tree": 1,
                "label": "TestTree",
                "is_talk_tree": false,
                "portrait_operand_bytes": 1,
                "entry_count": 2,
                "entries": [
                    {
                        "id": 0,
                        "text": "Professor!\nThank goodness you're safe!",
                        "segments": [
                            {"code": "0xF4", "ctrl": "portrait", "operands": [2], "id": 2},
                            {"text": "Professor!"},
                            {"code": "0xFC", "ctrl": "newline", "operands": []},
                            {"text": "Thank goodness you're safe!"}
                        ],
                        "pages": [{"lines": ["Professor!", "Thank goodness you're safe!"], "end": "end"}]
                    },
                    {
                        "id": 1,
                        "text": "Original line.",
                        "segments": [{"text": "Original line."}],
                        "pages": [{"lines": ["Original line."], "end": "end"}]
                    }
                ]
            }]
        })
        .to_string()
        .as_bytes(),
    );
    write_file(base, "dialogue/arrow.png", b"base-arrow-png");

    // Minimal valid BattleFiles (1 enemy id=10, 1 formation, 1 character, 1 equipment, 1 anim).
    let mut properties = serde_json::Map::new();
    for i in 0..crate::ELEMENT_SLOTS {
        properties.insert(format!("elem_{i}"), json!({"value": 2}));
    }
    write_file(
        base,
        "battle/enemies.json",
        json!({
            "format_version": version,
            "count": 1,
            "enemies": [{
                "id": 10,
                "symbol": "ZoranBult",
                "display_name": "ZORAN-BULT",
                "hp": 25,
                "stats": {
                    "strength": 18, "mental": 4, "agility": 6, "dexterity": 8,
                    "attack": 16, "defense": 2, "magic_defense": 0
                },
                "attack": {
                    "element": {"id": 1, "name": "physical"},
                    "status_effect": {"id": 0, "name": "none"}
                },
                "properties": properties,
                "ai": {
                    "condition_ids": [0, 0, 0, 0],
                    "conditional_ability_ids": [0, 0, 0, 0],
                    "regular_ability_ids": [0, 0, 0, 0, 0, 0, 0, 0]
                },
                "rewards": {"experience": 12, "meseta": 3}
            }]
        })
        .to_string()
        .as_bytes(),
    );
    write_file(
        base,
        "battle/formations.json",
        json!({
            "format_version": version,
            "formation_count": 1,
            "boss_formation_count": 0,
            "formations": [{
                "id": 0,
                "ambush_chance": 0, "run_chance": 0, "can_run": true,
                "drop_rate": 0, "drop_item": null, "count": 1,
                "count_matches_entries": true,
                "enemies": [{"slot": 1, "enemy_id": 10, "position": 14}]
            }],
            "boss_formations": [],
            "inline_formations": []
        })
        .to_string()
        .as_bytes(),
    );
    write_file(
        base,
        "battle/levels.json",
        json!({
            "format_version": version,
            "character_count": 0,
            "total_records": 0,
            "characters": []
        })
        .to_string()
        .as_bytes(),
    );
    write_file(
        base,
        "battle/abilities.json",
        json!({
            "format_version": version,
            "effects": {"table": "0x0061BE", "count": 44, "bounds_checked": false},
            "techniques": [], "skills": [], "enemy_skills": [], "item_effects": [], "combos": []
        })
        .to_string()
        .as_bytes(),
    );
    write_file(
        base,
        "battle/characters.json",
        json!({
            "format_version": version,
            "count": 0,
            "characters": []
        })
        .to_string()
        .as_bytes(),
    );
    write_file(
        base,
        "battle/equipment.json",
        json!({
            "format_version": version,
            "count": 0,
            "types": [],
            "items": []
        })
        .to_string()
        .as_bytes(),
    );
    write_file(
        base,
        "battle/enemy_animations.json",
        json!({
            "format_version": version,
            "count": 1,
            "source": {
                "label": "EnemyAttackOffs",
                "rom_offset": "0x000000",
                "table_end": "0x000004",
                "record_count": 1,
                "record_bytes": 4,
                "rom_size": 3145728,
                "rom_sha256": DIGEST,
                "grand_cross": 0,
                "reference_clone": "test",
                "reference_grand_cross": 1,
                "reference_role": "test",
                "tables": [{
                    "name": "EnemyAttackOffs",
                    "rom_offset": "0x000000",
                    "entry_count": 1,
                    "entry_bytes": 4,
                    "grand_cross": 0
                }]
            },
            "animations": [{
                "enemy_id": 10,
                "symbol": "ZoranBult",
                "display_name": "ZORAN-BULT",
                "routine_offset": "0x000000",
                "root_object_ids": [],
                "object_ids": [],
                "sfx_id": 0xA1,
                "sfx_name": "Slash",
                "dispatch": {
                    "kind": "Sound_Index",
                    "rom_offset": "0x000000",
                    "object_id": null
                },
                "sfx_writes": [{
                    "object_id": null,
                    "rom_offset": "0x000000",
                    "dispatch": "Sound_Index",
                    "sound_id": 0xA1,
                    "sound_name": "Slash"
                }],
                "frame_sequence": null,
                "movement_proven": false,
                "flash_timing_proven": false,
                "sprite_sheet_proven": false
            }],
            "census": {
                "enemy_count": 1,
                "distinct_attack_routines": 1,
                "exact_sfx": 1,
                "generic_sfx": 0,
                "frame_sequence_records": 0,
                "timed_frame_sequences": 0,
                "frame_sequence_deferred": 1,
                "movement_deferred": 1,
                "flash_timing": 0,
                "sprite_sheet_deferred": 1
            }
        })
        .to_string()
        .as_bytes(),
    );

    crate::GameData::load(base).expect("synthetic base pack GameData must load");
    crate::BattleFiles::load(base).expect("synthetic base pack BattleFiles must load");
}

fn snapshot_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    fn walk(root: &Path, current: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    walk(root, root, &mut map);
    map
}

#[test]
fn pack_overlay_resolves_in_memory_without_staging_dir_or_mutating_base() {
    let scratch = scratch_dir("full-e2e");
    let base = scratch.join("runtime-pack");
    let mods_dir = scratch.join("mods");
    write_synthetic_pack(&base);
    let base_before = snapshot_tree(&base);

    // Mod A (order 10): raises enemy 10 HP to 50, replaces Piata PNG & academy PNG, edits dialogue.
    let mod_a = mods_dir.join("mod-a");
    let mut man_a = ModManifest::new("mod-a", "Mod A", "1.0.0");
    man_a.order = 10;
    init_mod_dir(&mod_a, &man_a).unwrap();
    write_file(
        &mod_a,
        "patches/battle/enemies.json",
        br#"{"enemies":{"10":{"hp":50}}}"#,
    );
    write_file(
        &mod_a,
        "patches/dialogue/trees.json",
        br#"{
            "trees": {
                "1": {
                    "entries": {
                        "0": {"replace": [["Thank goodness you're safe!", "Glad you're okay!"]]},
                        "1": {"lines": [["Hello!", "Modded!"]]}
                    }
                }
            }
        }"#,
    );
    write_file(&mod_a, "assets/maps/010_Piata.png", b"mod-a-piata-png");
    write_file(
        &mod_a,
        "assets/maps/011_PiataAcademy_F1.png",
        b"mod-a-academy-png",
    );

    // Mod B (order 20): overrides enemy 10 HP to 99 and overrides Piata PNG again.
    let mod_b = mods_dir.join("mod-b");
    let mut man_b = ModManifest::new("mod-b", "Mod B", "2.0.0");
    man_b.order = 20;
    init_mod_dir(&mod_b, &man_b).unwrap();
    write_file(
        &mod_b,
        "patches/battle/enemies.json",
        br#"{"enemies":{"10":{"hp":99}}}"#,
    );
    write_file(&mod_b, "assets/maps/010_Piata.png", b"mod-b-piata-png");

    let discovered = discover_mods(&mods_dir).unwrap();
    assert_eq!(
        discovered
            .iter()
            .map(|m| m.manifest.id.as_str())
            .collect::<Vec<_>>(),
        vec!["mod-a", "mod-b"]
    );

    let (overlay, validated) = PackOverlay::load_validated(&base, &discovered).unwrap();

    // 1. No `mods/_active_pack` staging mirror created, and base_pack untouched.
    assert!(
        !mods_dir.join("_active_pack").exists(),
        "PackOverlay must never create mods/_active_pack"
    );
    assert_eq!(
        snapshot_tree(&base),
        base_before,
        "base pack must remain byte-for-byte untouched"
    );
    assert!(overlay.is_modded());
    assert_eq!(overlay.active_mods().len(), 2);
    assert_eq!(
        overlay.patched_files(),
        &["battle/enemies.json", "dialogue/trees.json"]
    );
    assert_eq!(
        overlay.replaced_assets(),
        &["maps/010_Piata.png", "maps/011_PiataAcademy_F1.png"]
    );

    // 2. Multi-mod table patch precedence (Mod B wins over Mod A) and dialogue shorthands.
    let battle = validated.battle_files.as_ref().unwrap();
    assert_eq!(battle.enemies.enemies[0].hp, 99);
    let dialogue = validated.game_data.dialogue().unwrap();
    let entry0 = dialogue.entry(1, 0).unwrap();
    assert!(entry0.text.contains("Glad you're okay!"), "{}", entry0.text);
    let entry1 = dialogue.entry(1, 1).unwrap();
    assert_eq!(entry1.text, "Hello!\nModded!");

    // 3. Live-palette map PNG reconciliation drops stale redirect in memory without touching base.
    assert!(overlay.map_redirect_is_stale("maps/010_Piata.png", "maps/010_Piata_indices.png"));
    assert!(
        !overlay
            .map_redirect_is_stale("maps/010_Piata_over.png", "maps/010_Piata_over_indices.png")
    );
    let piata = validated.game_data.map(MapId(0x010)).unwrap();
    assert!(!piata.map_update_images.contains_key("maps/010_Piata.png"));
    assert_eq!(
        piata
            .map_update_images
            .get("maps/010_Piata_over.png")
            .map(String::as_str),
        Some("maps/010_Piata_over_indices.png")
    );

    // 4. Asset resolution follows reverse load order before falling back to base_pack.
    assert_eq!(
        overlay.resolve_asset("maps/010_Piata.png"),
        Some(mod_b.join("assets/maps/010_Piata.png"))
    );
    assert_eq!(
        overlay.read_asset_bytes("maps/010_Piata.png").unwrap(),
        b"mod-b-piata-png"
    );
    assert_eq!(
        overlay.resolve_asset("maps/011_PiataAcademy_F1.png"),
        Some(mod_a.join("assets/maps/011_PiataAcademy_F1.png"))
    );
    assert_eq!(
        overlay
            .read_asset_bytes("maps/011_PiataAcademy_F1.png")
            .unwrap(),
        b"mod-a-academy-png"
    );
    assert_eq!(
        overlay.resolve_asset("maps/010_Piata_over.png"),
        Some(base.join("maps/010_Piata_over.png"))
    );
    assert_eq!(
        overlay.read_asset_bytes("maps/010_Piata_over.png").unwrap(),
        b"base-piata-over"
    );

    // 5. In-memory patched JSONs are NOT exposed as raw disk paths via resolve_asset,
    //    preventing callers from accidentally reading unpatched base JSON.
    assert_eq!(overlay.resolve_asset("battle/enemies.json"), None);
    assert_eq!(overlay.resolve_asset("maps/010_Piata.json"), None);
    assert!(
        overlay
            .read_text("battle/enemies.json")
            .unwrap()
            .contains("\"hp\":99")
    );

    // 6. Path traversal & absolute paths are rejected cleanly.
    let parent_escape = ["..", "secret.txt"].join("/");
    for bad in [
        "",
        ".",
        "..",
        parent_escape.as_str(),
        "maps/../../secret.txt",
        "/etc/passwd",
        "\\Windows\\System32",
        "C:/Windows/win.ini",
    ] {
        assert_eq!(overlay.resolve_asset(bad), None, "must reject {bad:?}");
        assert!(!overlay.is_file(bad), "is_file must reject {bad:?}");
        assert!(
            overlay.read_asset_bytes(bad).is_err(),
            "read_asset_bytes must reject {bad:?}"
        );
        assert!(
            overlay.read_text(bad).is_err(),
            "read_text must reject {bad:?}"
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn pack_overlay_attributes_schema_and_typo_errors_to_offending_mod() {
    let scratch = scratch_dir("error-attr");
    let base = scratch.join("runtime-pack");
    let mods_dir = scratch.join("mods");
    write_synthetic_pack(&base);

    // Good Mod A + Bad Mod B (out-of-range u16 HP): must blame Mod B, not Mod A.
    let mod_a = mods_dir.join("good-a");
    let mut man_a = ModManifest::new("good-a", "Good A", "1.0.0");
    man_a.order = 10;
    init_mod_dir(&mod_a, &man_a).unwrap();
    write_file(
        &mod_a,
        "patches/battle/enemies.json",
        br#"{"enemies":{"10":{"hp":50}}}"#,
    );

    let mod_b = mods_dir.join("bad-b");
    let mut man_b = ModManifest::new("bad-b", "Bad B", "1.0.0");
    man_b.order = 20;
    init_mod_dir(&mod_b, &man_b).unwrap();
    write_file(
        &mod_b,
        "patches/battle/enemies.json",
        br#"{"enemies":{"10":{"hp":70000}}}"#,
    );

    let discovered = discover_mods(&mods_dir).unwrap();
    let err = PackOverlay::load(&base, &discovered).expect_err("HP 70000 must fail closed");
    assert_eq!(err.mod_id, "bad-b");
    assert_eq!(err.file, "battle/enemies.json");
    assert!(err.detail.contains("70000"), "{err}");
    assert!(err.detail.contains("u16"), "{err}");
    assert!(!mods_dir.join("_active_pack").exists());

    // Typo field ("HP": 99) fails closed naming mod, file, and field.
    write_file(
        &mod_b,
        "patches/battle/enemies.json",
        br#"{"enemies":{"10":{"HP":99}}}"#,
    );
    let typo_err = PackOverlay::load(&base, &discovered).expect_err("typo field HP must fail");
    assert_eq!(typo_err.mod_id, "bad-b");
    assert!(typo_err.file.contains("battle/enemies.json"), "{typo_err}");
    assert!(typo_err.detail.contains("HP"), "{typo_err}");

    let _ = fs::remove_dir_all(&scratch);
}

#[cfg(unix)]
#[test]
fn pack_overlay_rejects_symlinks_in_mod_assets() {
    let scratch = scratch_dir("asset-symlink");
    let base = scratch.join("runtime-pack");
    write_synthetic_pack(&base);

    let mod_dir = scratch.join("mods/sym-asset");
    init_mod_dir(&mod_dir, &ModManifest::new("sym-asset", "Sym", "1.0.0")).unwrap();
    let external = scratch.join("outside.png");
    fs::write(&external, b"outside").unwrap();
    fs::create_dir_all(mod_dir.join("assets/maps")).unwrap();
    std::os::unix::fs::symlink(&external, mod_dir.join("assets/maps/010_Piata.png")).unwrap();

    let err = PackOverlay::load(&base, &[load_mod(&mod_dir).unwrap()])
        .expect_err("symlink inside mod assets/ must be rejected");
    assert_eq!(err.mod_id, "sym-asset");
    assert!(err.detail.contains("symlinks"), "{err}");

    let _ = fs::remove_dir_all(&scratch);
}
