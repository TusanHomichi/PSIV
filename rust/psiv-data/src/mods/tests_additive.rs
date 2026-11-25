//! Tests for explicit additive record creation (`"$add"`), reverse diff
//! round-tripping, multi-mod ordering & conflict detection, full load-time
//! cross-reference validation, and published JSON Schema enforcement (#792).

use super::overlay_tests::write_synthetic_pack;
use super::schema_tests::{load_published_schema, validate_against_schema};
use super::*;
use crate::{BattleFiles, GameData, MapId};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static ADD_TEST_ID: AtomicUsize = AtomicUsize::new(1);

fn temp_dir(label: &str) -> PathBuf {
    let n = ADD_TEST_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "psiv-mod-additive-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_json(root: &Path, rel: &str, value: &Value) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
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

fn sample_npc(index: u32, dialogue_id: u16) -> Value {
    json!({
        "index": index,
        "record_offset": "0x000000",
        "object_id": 60,
        "symbol": "NPCType2",
        "x_pixels": 16,
        "y_pixels": 16,
        "x_cell": 1,
        "y_cell": 2,
        "facing": {"id": 0, "name": "down"},
        "dialogue_id": dialogue_id,
        "sprite": null,
        "sprite_reason": "custom mod npc",
        "art_tile": 0,
        "interactable": true,
        "camera_bypass": false
    })
}

fn sample_chest(index: u32, item_id: u16) -> Value {
    json!({
        "index": index,
        "record_offset": "0x000000",
        "x_cell": 2,
        "y_cell": 3,
        "x_pixels": 32,
        "y_pixels": 32,
        "white_chest": false,
        "object_symbol": "TreasureChest",
        "sprite": null,
        "contents_type": "item",
        "item_id": item_id,
        "item_symbol": "Monomate",
        "meseta": null,
        "chest_flag": 250
    })
}

fn sample_warp(index: u32, target_id: u16, dest_x: u32, dest_y: u32) -> Value {
    json!({
        "index": index,
        "table": 2,
        "trigger": "map_change_tile",
        "record_offset": "0x000000",
        "range": {"id": 3, "name": "XYExact"},
        "source": {"x_byte": 0, "y_byte": 0, "x_cell": 0, "y_cell": 0},
        "rect": {"x": 0, "y": 0, "width": 1, "height": 1},
        "target": {"id": target_id, "symbol": "PiataAcademy"},
        "destination": {"x_cell": dest_x, "y_cell": dest_y},
        "facing": {"id": 4, "name": "up"},
        "character_alignment": 0
    })
}

fn sample_formation(id: u16, enemy_id: u16) -> Value {
    json!({
        "id": id,
        "ambush_chance": 0,
        "run_chance": 0,
        "can_run": true,
        "drop_rate": 8,
        "drop_item": 125,
        "count": 2,
        "count_matches_entries": true,
        "enemies": [
            {"slot": 1, "enemy_id": enemy_id, "position": 14},
            {"slot": 2, "enemy_id": enemy_id, "position": 26}
        ]
    })
}

#[test]
fn additive_records_round_trip_across_all_five_tables_on_synthetic_pack() {
    let scratch = temp_dir("all-five");
    let base = scratch.join("base");
    write_synthetic_pack(&base);

    let mod_dir = scratch.join("mods/content-pack");
    init_mod_dir(
        &mod_dir,
        &ModManifest::new("content-pack", "Content Pack", "1.0.0"),
    )
    .unwrap();

    // 1. Add two dialogue entries (lines shorthand + full entry with portrait).
    write_json(
        &mod_dir,
        "patches/dialogue/trees.json",
        &json!({
            "trees": {
                "1": {
                    "entries": {
                        "$add": [
                            {"id": 2, "lines": ["Welcome to Piata!", "Try the new chest."]},
                            {
                                "id": 3,
                                "text": "Portrait line!",
                                "segments": [
                                    {"code": "0xF4", "ctrl": "portrait", "operands": [2], "id": 2},
                                    {"text": "Portrait line!"}
                                ],
                                "pages": [{"lines": ["Portrait line!"], "end": "end"}]
                            }
                        ]
                    }
                }
            }
        }),
    );

    // 2. Add a map NPC (referencing newly added dialogue entry 2), a treasure chest, and a warp.
    write_json(
        &mod_dir,
        "patches/maps/010_Piata.json",
        &json!({
            "npcs": {"$add": [sample_npc(1, 2)]},
            "treasure_chests": {"$add": [sample_chest(1, 125)]},
            "warps": {"$add": [sample_warp(1, 17, 1, 1)]}
        }),
    );

    // 3. Add a battle formation (id 1, referencing existing enemy 10 and drop_item 125).
    write_json(
        &mod_dir,
        "patches/battle/formations.json",
        &json!({
            "formations": {"$add": [sample_formation(1, 10)]}
        }),
    );

    let loaded = load_mod(&mod_dir).unwrap();
    let (overlay, validated) = PackOverlay::load_validated(&base, &[loaded]).unwrap();

    // Verify loaded GameData + BattleFiles + auto-reconciled counts.
    let dialogue = validated.game_data.dialogue().unwrap();
    assert_eq!(dialogue.trees.entry_count, 4);
    let tree1 = dialogue.tree(1).unwrap();
    assert_eq!(tree1.entry_count, 4);
    assert_eq!(
        tree1.entry(2).unwrap().text,
        "Welcome to Piata!\nTry the new chest."
    );
    assert_eq!(tree1.entry(3).unwrap().text, "Portrait line!");

    let piata = validated.game_data.map(MapId(0x010)).unwrap();
    assert_eq!(piata.npcs.len(), 2);
    assert_eq!(piata.npcs[1].index, 1);
    assert_eq!(piata.npcs[1].dialogue_id, 2);
    assert_eq!(piata.treasure_chests.len(), 2);
    assert_eq!(piata.treasure_chests[1].index, 1);
    assert_eq!(piata.warps.len(), 2);
    assert_eq!(piata.warps[1].index, 1);

    let battle = validated.battle_files.as_ref().unwrap();
    assert_eq!(battle.formations.formation_count, 2);
    assert_eq!(battle.formations.formations.len(), 2);
    assert_eq!(battle.formations.formations[1].id, Some(1));

    // Shareability audit produces zero warnings or whole-array notices.
    let notices = audit_mod_shareability(&base, &load_mod(&mod_dir).unwrap()).unwrap();
    assert!(notices.is_empty(), "expected zero notices, got {notices:?}");

    // Strict default diff_packs exports all 5 additive tables cleanly and round-trips identically.
    let edited_copy = scratch.join("edited-copy");
    tests::materialize_overlay_for_diff_test(&overlay, &edited_copy);
    let exported = scratch.join("exported");
    let summary = diff_packs(
        &base,
        &edited_copy,
        &exported,
        &ModManifest::new("exported", "Exported", "1.0.0"),
    )
    .expect("strict diff_packs must export appended additive records");
    assert_eq!(
        summary.patches,
        vec![
            "battle/formations.json",
            "dialogue/trees.json",
            "maps/010_Piata.json"
        ]
    );
    assert!(summary.derived_items.is_empty());

    // Derived counts must be omitted from exported patches so multi-mod stacking composes cleanly.
    let form_patch: Value = serde_json::from_str(
        &fs::read_to_string(exported.join("patches/battle/formations.json")).unwrap(),
    )
    .unwrap();
    assert!(form_patch.get("formation_count").is_none(), "{form_patch}");
    assert!(form_patch["formations"]["$add"].is_array());

    let tree_patch: Value = serde_json::from_str(
        &fs::read_to_string(exported.join("patches/dialogue/trees.json")).unwrap(),
    )
    .unwrap();
    assert!(tree_patch.get("entry_count").is_none(), "{tree_patch}");
    assert!(tree_patch["trees"]["1"].get("entry_count").is_none());
    // Plain entry 2 synthesizes back to `lines` shorthand, while portrait entry 3 keeps full form.
    assert_eq!(
        tree_patch["trees"]["1"]["entries"]["$add"][0],
        json!({"id": 2, "lines": ["Welcome to Piata!", "Try the new chest."]})
    );
    assert!(tree_patch["trees"]["1"]["entries"]["$add"][1]["segments"].is_array());

    // Re-applying the exported mod produces identical validated GameData & BattleFiles.
    let (_, revalidated) =
        PackOverlay::load_validated(&base, &[load_mod(&exported).unwrap()]).unwrap();
    assert_eq!(revalidated.game_data, validated.game_data);
    assert_eq!(revalidated.battle_files, validated.battle_files);

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn multi_mod_ordering_cross_patching_and_conflict_errors() {
    let scratch = temp_dir("multi-mod");
    let base = scratch.join("base");
    let mods_dir = scratch.join("mods");
    write_synthetic_pack(&base);

    // Mod A (order 10): adds dialogue 2, NPC 1, formation 10.
    let mod_a = mods_dir.join("mod-a");
    let mut man_a = ModManifest::new("mod-a", "Mod A", "1.0.0");
    man_a.order = 10;
    init_mod_dir(&mod_a, &man_a).unwrap();
    write_json(
        &mod_a,
        "patches/dialogue/trees.json",
        &json!({"trees": {"1": {"entries": {"$add": [{"id": 2, "lines": ["From A"]}]}}}}),
    );
    write_json(
        &mod_a,
        "patches/maps/010_Piata.json",
        &json!({"npcs": {"$add": [sample_npc(1, 2)]}}),
    );
    write_json(
        &mod_a,
        "patches/battle/formations.json",
        &json!({"formations": {"$add": [sample_formation(10, 10)]}}),
    );

    // Mod B (order 20): patches Mod A's dialogue 2 and formation 10, AND adds dialogue 3 + formation 20.
    let mod_b = mods_dir.join("mod-b");
    let mut man_b = ModManifest::new("mod-b", "Mod B", "1.0.0");
    man_b.order = 20;
    init_mod_dir(&mod_b, &man_b).unwrap();
    write_json(
        &mod_b,
        "patches/dialogue/trees.json",
        &json!({
            "trees": {
                "1": {
                    "entries": {
                        "2": {"lines": ["Patched by B"]},
                        "$add": [{"id": 3, "lines": ["Added by B"]}]
                    }
                }
            }
        }),
    );
    write_json(
        &mod_b,
        "patches/battle/formations.json",
        &json!({
            "formations": {
                "10": {"run_chance": 42},
                "$add": [sample_formation(20, 10)]
            }
        }),
    );

    let discovered = discover_mods(&mods_dir).unwrap();
    let (_, validated) = PackOverlay::load_validated(&base, &discovered).unwrap();

    // Deterministic order: base records first, then Mod A's additions, then Mod B's additions.
    let form_ids: Vec<Option<u16>> = validated
        .battle_files
        .as_ref()
        .unwrap()
        .formations
        .formations
        .iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(form_ids, vec![Some(0), Some(10), Some(20)]);
    assert_eq!(
        validated
            .battle_files
            .as_ref()
            .unwrap()
            .formations
            .formation_count,
        3
    );
    assert_eq!(
        validated
            .battle_files
            .as_ref()
            .unwrap()
            .formations
            .formations[1]
            .run_chance,
        42
    );
    let dlg = validated.game_data.dialogue().unwrap();
    assert_eq!(dlg.trees.entry_count, 4);
    assert_eq!(dlg.entry(1, 2).unwrap().text, "Patched by B");
    assert_eq!(dlg.entry(1, 3).unwrap().text, "Added by B");

    // Negative control 1: Mod C tries to `$add` formation 10 (already added by Mod A) -> fails closed!
    let mod_c = mods_dir.join("mod-c");
    let mut man_c = ModManifest::new("mod-c", "Mod C", "1.0.0");
    man_c.order = 30;
    init_mod_dir(&mod_c, &man_c).unwrap();
    write_json(
        &mod_c,
        "patches/battle/formations.json",
        &json!({"formations": {"$add": [sample_formation(10, 10)]}}),
    );
    let conflict = PackOverlay::load(&base, &discover_mods(&mods_dir).unwrap())
        .expect_err("cross-mod duplicate $add id must fail closed");
    assert_eq!(conflict.mod_id, "mod-c");
    assert!(
        conflict.file.contains("battle/formations.json"),
        "{conflict}"
    );
    assert!(conflict.detail.contains("id=10"), "{conflict}");
    assert!(conflict.detail.contains("mod-a"), "{conflict}");

    // Negative control 2: Mod C tries to `$add` formation 0 (already in base pack) -> fails closed!
    write_json(
        &mod_c,
        "patches/battle/formations.json",
        &json!({"formations": {"$add": [sample_formation(0, 10)]}}),
    );
    let base_conflict = PackOverlay::load(&base, &discover_mods(&mods_dir).unwrap())
        .expect_err("base-pack duplicate $add id must fail closed");
    assert_eq!(base_conflict.mod_id, "mod-c");
    assert!(
        base_conflict.file.contains("battle/formations.json"),
        "{base_conflict}"
    );
    assert!(base_conflict.detail.contains("id=0"), "{base_conflict}");
    assert!(
        base_conflict.detail.contains("base pack"),
        "{base_conflict}"
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn additive_negative_controls_fail_closed_naming_mod_file_and_record_id() {
    let scratch = temp_dir("negatives");
    let base = scratch.join("base");
    write_synthetic_pack(&base);

    let run_single_mod = |name: &str, rel: &str, patch: Value| -> ModError {
        let dir = scratch.join(format!("mod-{name}"));
        let _ = fs::remove_dir_all(&dir);
        init_mod_dir(&dir, &ModManifest::new(name, name, "1.0.0")).unwrap();
        write_json(&dir, rel, &patch);
        PackOverlay::load(&base, &[load_mod(&dir).unwrap()])
            .expect_err(&format!("expected failure for {name}"))
    };

    // 1. Typo ID in ordinary edit patch NEVER silently creates a record.
    let err = run_single_mod(
        "typo-edit-id",
        "patches/battle/formations.json",
        json!({"formations": {"999": {"run_chance": 5}}}),
    );
    assert_eq!(err.mod_id, "typo-edit-id");
    assert!(err.file.contains("battle/formations.json"), "{err}");
    assert!(err.detail.contains("999"), "{err}");

    // 2. `$add` in a non-additive table (`battle/enemies.json`) fails closed.
    let err = run_single_mod(
        "non-additive-table",
        "patches/battle/enemies.json",
        json!({"enemies": {"$add": [{"id": 99, "hp": 10}]}}),
    );
    assert_eq!(err.mod_id, "non-additive-table");
    assert!(err.file.contains("battle/enemies.json"), "{err}");
    assert!(err.detail.contains("$add"), "{err}");

    // 3. Incomplete required fields on added formation fails closed naming record id.
    let err = run_single_mod(
        "incomplete-formation",
        "patches/battle/formations.json",
        json!({"formations": {"$add": [{"id": 504, "ambush_chance": 0}]}}),
    );
    assert_eq!(err.mod_id, "incomplete-formation");
    assert!(err.file.contains("battle/formations.json"), "{err}");
    assert!(err.detail.contains("id=504"), "{err}");
    assert!(err.detail.contains("missing field"), "{err}");

    // 4. Incomplete required fields on added NPC fails closed naming record index.
    let err = run_single_mod(
        "incomplete-npc",
        "patches/maps/010_Piata.json",
        json!({"npcs": {"$add": [{"index": 1, "object_id": 60}]}}),
    );
    assert_eq!(err.mod_id, "incomplete-npc");
    assert!(err.file.contains("maps/010_Piata.json"), "{err}");
    assert!(err.detail.contains("index=1"), "{err}");
    assert!(err.detail.contains("missing field"), "{err}");

    // 5. Dangling enemy_id on added formation fails closed naming mod, file, and formation id.
    let err = run_single_mod(
        "bad-enemy-ref",
        "patches/battle/formations.json",
        json!({"formations": {"$add": [sample_formation(504, 999)]}}),
    );
    assert_eq!(err.mod_id, "bad-enemy-ref");
    assert_eq!(err.file, "battle/formations.json");
    assert!(err.detail.contains("504"), "{err}");
    assert!(err.detail.contains("999"), "{err}");

    // 6. Dangling drop_item on added formation fails closed naming mod, file, and formation id.
    let mut bad_drop = sample_formation(505, 10);
    bad_drop["drop_item"] = json!(250);
    let err = run_single_mod(
        "bad-drop-item",
        "patches/battle/formations.json",
        json!({"formations": {"$add": [bad_drop]}}),
    );
    assert_eq!(err.mod_id, "bad-drop-item");
    assert_eq!(err.file, "battle/formations.json");
    assert!(err.detail.contains("505"), "{err}");
    assert!(err.detail.contains("250"), "{err}");

    // 7. Dangling item_id on added chest fails closed naming mod, map file, and chest index.
    let err = run_single_mod(
        "bad-chest-item",
        "patches/maps/010_Piata.json",
        json!({"treasure_chests": {"$add": [sample_chest(1, 250)]}}),
    );
    assert_eq!(err.mod_id, "bad-chest-item");
    assert_eq!(err.file, "maps/010_Piata.json");
    assert!(err.detail.contains("index 1"), "{err}");
    assert!(err.detail.contains("250"), "{err}");

    // 8. Dangling dialogue_id on added NPC fails closed naming mod, map file, and npc index.
    let err = run_single_mod(
        "bad-npc-dialogue",
        "patches/maps/010_Piata.json",
        json!({"npcs": {"$add": [sample_npc(1, 99)]}}),
    );
    assert_eq!(err.mod_id, "bad-npc-dialogue");
    assert_eq!(err.file, "maps/010_Piata.json");
    assert!(err.detail.contains("index 1"), "{err}");
    assert!(err.detail.contains("99"), "{err}");

    // 9. Missing sprite sheet on added NPC fails closed naming mod, map file, and npc index.
    let mut bad_sprite_npc = sample_npc(1, 0);
    bad_sprite_npc["sprite_reason"] = json!(null);
    bad_sprite_npc["sprite"] = json!({
        "sheet": "NoSuchSheet",
        "sheets": "sprites/npcs.json",
        "facing": "down",
        "idle_sequence": "idle_down",
        "walk_sequence": "walk_down"
    });
    let err = run_single_mod(
        "bad-npc-sprite",
        "patches/maps/010_Piata.json",
        json!({"npcs": {"$add": [bad_sprite_npc]}}),
    );
    assert_eq!(err.mod_id, "bad-npc-sprite");
    assert_eq!(err.file, "maps/010_Piata.json");
    assert!(err.detail.contains("npc 1"), "{err}");
    assert!(err.detail.contains("NoSuchSheet"), "{err}");

    // 10. Invalid object_id on added NPC fails closed naming mod, map file, and npc index.
    let mut bad_obj_npc = sample_npc(1, 0);
    bad_obj_npc["object_id"] = json!(9999);
    let err = run_single_mod(
        "bad-npc-obj",
        "patches/maps/010_Piata.json",
        json!({"npcs": {"$add": [bad_obj_npc]}}),
    );
    assert_eq!(err.mod_id, "bad-npc-obj");
    assert_eq!(err.file, "maps/010_Piata.json");
    assert!(err.detail.contains("index 1"), "{err}");
    assert!(err.detail.contains("9999"), "{err}");

    // 11. Out-of-bounds map coordinates on added NPC fails closed naming mod, map file, and npc index.
    let mut oob_npc = sample_npc(1, 0);
    oob_npc["x_cell"] = json!(99);
    let err = run_single_mod(
        "oob-npc",
        "patches/maps/010_Piata.json",
        json!({"npcs": {"$add": [oob_npc]}}),
    );
    assert_eq!(err.mod_id, "oob-npc");
    assert_eq!(err.file, "maps/010_Piata.json");
    assert!(err.detail.contains("index 1"), "{err}");

    // 12. Invalid destination map ID on added warp fails closed naming mod, map file, and warp index.
    let err = run_single_mod(
        "bad-warp-target",
        "patches/maps/010_Piata.json",
        json!({"warps": {"$add": [sample_warp(1, 999, 1, 1)]}}),
    );
    assert_eq!(err.mod_id, "bad-warp-target");
    assert_eq!(err.file, "maps/010_Piata.json");
    assert!(err.detail.contains("index 1"), "{err}");
    assert!(
        err.detail.contains("0x3e7") || err.detail.contains("0x3E7"),
        "{err}"
    );

    // 13. Multi-mod attribution: Mod A adds broken map NPC, Mod B validly patches enemies.json ->
    //     error MUST blame Mod A and `maps/010_Piata.json`, not Mod B!
    let mod_a = scratch.join("attr-a");
    let mut man_a = ModManifest::new("attr-a", "Attr A", "1.0.0");
    man_a.order = 10;
    init_mod_dir(&mod_a, &man_a).unwrap();
    write_json(
        &mod_a,
        "patches/maps/010_Piata.json",
        &json!({"treasure_chests": {"$add": [sample_chest(1, 250)]}}),
    );
    let mod_b = scratch.join("attr-b");
    let mut man_b = ModManifest::new("attr-b", "Attr B", "1.0.0");
    man_b.order = 20;
    init_mod_dir(&mod_b, &man_b).unwrap();
    write_json(
        &mod_b,
        "patches/battle/enemies.json",
        &json!({"enemies": {"10": {"hp": 30}}}),
    );
    let attr_err = PackOverlay::load(
        &base,
        &[load_mod(&mod_a).unwrap(), load_mod(&mod_b).unwrap()],
    )
    .expect_err("broken chest item_id must fail");
    assert_eq!(attr_err.mod_id, "attr-a");
    assert_eq!(attr_err.file, "maps/010_Piata.json");
    assert!(attr_err.detail.contains("index 1"), "{attr_err}");

    // 14. Diffing a mid-list insertion or deletion or reorder still fails closed.
    let base_map = json!({"npcs": [sample_npc(0, 0)]});
    let inserted_front = json!({"npcs": [sample_npc(1, 0), sample_npc(0, 0)]});
    let front_err = diff_json(&base_map, &inserted_front, "maps/010_Piata.json")
        .expect_err("inserting before base records must fail diff");
    assert!(front_err.contains("appended"), "{front_err}");

    // 15. Non-dense dialogue entry IDs in `$add` (gap or out-of-order) fail immediately in both
    //     apply_json_patch and diff_json.
    let mut base_trees = json!({
        "trees": [{
            "tree": 1,
            "entry_count": 1,
            "entries": [{
                "id": 0,
                "text": "Hi",
                "segments": [{"text": "Hi"}],
                "pages": [{"lines": ["Hi"], "end": "end"}]
            }]
        }]
    });
    let gap_patch = json!({
        "trees": {
            "1": {
                "entries": {
                    "$add": [{"id": 5, "lines": ["Skipped ID"]}]
                }
            }
        }
    });
    let gap_err = apply_json_patch(&mut base_trees, &gap_patch, "dialogue/trees.json")
        .expect_err("non-dense dialogue entry id must fail patch apply");
    assert!(gap_err.contains("dense"), "{gap_err}");
    assert!(gap_err.contains("expected id=1"), "{gap_err}");

    let edited_gap_trees = json!({
        "trees": [{
            "tree": 1,
            "entry_count": 2,
            "entries": [
                {
                    "id": 0,
                    "text": "Hi",
                    "segments": [{"text": "Hi"}],
                    "pages": [{"lines": ["Hi"], "end": "end"}]
                },
                {
                    "id": 5,
                    "text": "Gap",
                    "segments": [{"text": "Gap"}],
                    "pages": [{"lines": ["Gap"], "end": "end"}]
                }
            ]
        }]
    });
    let diff_gap_err = diff_json(&base_trees, &edited_gap_trees, "dialogue/trees.json")
        .expect_err("non-dense dialogue entry id must fail diff");
    assert!(diff_gap_err.contains("dense"), "{diff_gap_err}");

    // 16. MileHouse silent-NPC exemption (0x020/0x022, NPCs 2/3, dialogue_id 110) only applies
    //     when the map uses DialogueTree4; repointing dialogue_tree away from 4 must fail closed.
    let mile_base = scratch.join("mile-base");
    write_synthetic_pack(&mile_base);
    let mut mile_manifest: Value =
        serde_json::from_str(&fs::read_to_string(mile_base.join("manifest.json")).unwrap())
            .unwrap();
    let mut mile_entry = mile_manifest["maps"][0].clone();
    mile_entry["id"] = json!(0x020);
    mile_entry["id_hex"] = json!("0x020");
    mile_entry["name"] = json!("MileHouse1");
    mile_entry["json"] = json!("maps/020_MileHouse1.json");
    mile_manifest["maps"]
        .as_array_mut()
        .unwrap()
        .push(mile_entry);
    fs::write(
        mile_base.join("manifest.json"),
        serde_json::to_string_pretty(&mile_manifest).unwrap(),
    )
    .unwrap();
    let mut mile_map: Value =
        serde_json::from_str(&fs::read_to_string(mile_base.join("maps/010_Piata.json")).unwrap())
            .unwrap();
    mile_map["id"] = json!(0x020);
    mile_map["id_hex"] = json!("0x020");
    mile_map["name"] = json!("MileHouse1");
    mile_map["dialogue_tree"] = json!(1);
    mile_map["npcs"] = json!([sample_npc(2, 110)]);
    fs::write(
        mile_base.join("maps/020_MileHouse1.json"),
        serde_json::to_string_pretty(&mile_map).unwrap(),
    )
    .unwrap();
    let mile_err = PackOverlay::load_validated(&mile_base, &[])
        .expect_err("MileHouse1 with dialogue_tree != 4 must reject dangling dialogue_id 110");
    assert!(mile_err.detail.contains("dialogue_id 110"), "{mile_err}");

    // 17. Non-positional indexes on map npcs, treasure_chests, and warps fail closed in both
    //     apply_json_patch and diff_json.
    for (field, bad_record, base_record) in [
        ("npcs", sample_npc(5, 0), sample_npc(0, 0)),
        (
            "treasure_chests",
            sample_chest(5, 125),
            sample_chest(0, 125),
        ),
        ("warps", sample_warp(5, 17, 1, 1), sample_warp(0, 17, 1, 1)),
    ] {
        let mut target = json!({ field: [base_record.clone()] });
        let patch = json!({ field: { "$add": [bad_record.clone()] } });
        let err = apply_json_patch(&mut target, &patch, "maps/010_Piata.json")
            .expect_err("non-positional map index must fail patch apply");
        assert!(err.contains("expected index=1"), "{field}: {err}");

        let edited = json!({ field: [base_record, bad_record] });
        let diff_err = diff_json(&target, &edited, "maps/010_Piata.json")
            .expect_err("non-positional map index must fail diff");
        assert!(diff_err.contains("`index`=1"), "{field}: {diff_err}");
    }

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn published_schemas_validate_additive_records_and_reject_incomplete_ones() {
    let trees_schema = load_published_schema("patches/dialogue/trees.schema.json");
    let map_schema = load_published_schema("patches/map.schema.json");
    let formations_schema = load_published_schema("patches/battle/formations.schema.json");
    let enemies_schema = load_published_schema("patches/battle/enemies.schema.json");

    // Positive controls: valid `$add` records for all 5 tables pass published JSON Schemas.
    validate_against_schema(
        &trees_schema,
        &json!({
            "trees": {
                "1": {
                    "entries": {
                        "$add": [
                            {"id": 111, "lines": ["Hello!", "New line!"]},
                            {
                                "id": 112,
                                "text": "Full entry",
                                "segments": [{"text": "Full entry"}],
                                "pages": [{"lines": ["Full entry"], "end": "end"}]
                            }
                        ]
                    }
                }
            }
        }),
    )
    .unwrap();
    validate_against_schema(
        &map_schema,
        &json!({
            "npcs": {"$add": [sample_npc(12, 0)]},
            "treasure_chests": {"$add": [sample_chest(5, 125)]},
            "warps": {"$add": [sample_warp(4, 17, 1, 1)]}
        }),
    )
    .unwrap();
    validate_against_schema(
        &formations_schema,
        &json!({"formations": {"$add": [sample_formation(504, 10)]}}),
    )
    .unwrap();

    // Negative controls: incomplete `$add` records, empty `$add`, and `$add` on non-additive tables fail.
    for (label, schema, doc) in [
        (
            "non-additive enemies $add",
            &enemies_schema,
            json!({"enemies": {"$add": [{"id": 999}]}}),
        ),
        (
            "empty $add array",
            &formations_schema,
            json!({"formations": {"$add": []}}),
        ),
        (
            "formation missing id",
            &formations_schema,
            json!({"formations": {"$add": [{
                "ambush_chance": 0, "run_chance": 0, "can_run": true,
                "drop_rate": 0, "count": 1, "count_matches_entries": true,
                "enemies": [{"slot": 1, "enemy_id": 10, "position": 14}]
            }]}}),
        ),
        (
            "formation missing required fields",
            &formations_schema,
            json!({"formations": {"$add": [{"id": 504}]}}),
        ),
        (
            "formation empty enemies",
            &formations_schema,
            json!({"formations": {"$add": [{
                "id": 504, "ambush_chance": 0, "run_chance": 0, "can_run": true,
                "drop_rate": 0, "count": 0, "count_matches_entries": true, "enemies": []
            }]}}),
        ),
        (
            "npc missing required fields",
            &map_schema,
            json!({"npcs": {"$add": [{"index": 12, "object_id": 60}]}}),
        ),
        (
            "chest missing required fields",
            &map_schema,
            json!({"treasure_chests": {"$add": [{"index": 5, "contents_type": "meseta"}]}}),
        ),
        (
            "warp missing required fields",
            &map_schema,
            json!({"warps": {"$add": [{"index": 4, "table": 2}]}}),
        ),
        (
            "dialogue incomplete full entry",
            &trees_schema,
            json!({"trees": {"1": {"entries": {"$add": [{"id": 111, "text": "Only text"}]}}}}),
        ),
        (
            "dialogue lines mixed with text",
            &trees_schema,
            json!({"trees": {"1": {"entries": {"$add": [{"id": 111, "lines": ["Hi"], "text": "Hi"}]}}}}),
        ),
    ] {
        validate_against_schema(schema, &doc)
            .expect_err(&format!("published schema must reject {label}: {doc}"));
    }
}

#[test]
fn real_pack_additive_records_load_and_diff_round_trip() {
    let Some(base) = real_pack() else {
        eprintln!("runtime-pack absent; skipping real-pack additive test");
        return;
    };

    let base_game = GameData::load(&base).unwrap();
    let base_battle = BattleFiles::load(&base).unwrap();
    let piata = base_game.map(MapId(0x010)).unwrap();
    let dlg_tree_id = piata.dialogue_tree;
    let next_dlg_id = base_game
        .dialogue()
        .unwrap()
        .tree(dlg_tree_id)
        .unwrap()
        .entries
        .len() as u16;
    let next_npc_idx = piata.npcs.len() as u32;
    let next_chest_idx = piata.treasure_chests.len() as u32;
    let next_warp_idx = piata.warps.len() as u32;
    let next_form_id = base_battle.formations.formations.len() as u16;

    // Pick a real existing NPC sprite from Piata so we also exercise real sprite cross-refs.
    let real_sprite = piata
        .npcs
        .iter()
        .find_map(|n| n.sprite.as_ref())
        .expect("Piata has sprite-bearing NPCs");

    let scratch = temp_dir("real-pack");
    let mod_dir = scratch.join("mods/real-additive");
    init_mod_dir(
        &mod_dir,
        &ModManifest::new("real-additive", "Real Additive", "1.0.0"),
    )
    .unwrap();

    write_json(
        &mod_dir,
        "patches/dialogue/trees.json",
        &json!({
            "trees": {
                dlg_tree_id.to_string(): {
                    "entries": {
                        "$add": [{"id": next_dlg_id, "lines": ["Custom Piata NPC!", "Added by mod."]}]
                    }
                }
            }
        }),
    );

    let mut new_npc = sample_npc(next_npc_idx, next_dlg_id);
    new_npc["sprite_reason"] = json!(null);
    new_npc["sprite"] = serde_json::to_value(real_sprite).unwrap();
    write_json(
        &mod_dir,
        "patches/maps/010_Piata.json",
        &json!({
            "npcs": {"$add": [new_npc]},
            "treasure_chests": {"$add": [sample_chest(next_chest_idx, 125)]},
            "warps": {"$add": [sample_warp(next_warp_idx, 0x011, 8, 8)]}
        }),
    );
    write_json(
        &mod_dir,
        "patches/battle/formations.json",
        &json!({"formations": {"$add": [sample_formation(next_form_id, 10)]}}),
    );

    let (overlay, validated) =
        PackOverlay::load_validated(&base, &[load_mod(&mod_dir).unwrap()]).unwrap();
    assert_eq!(
        validated
            .game_data
            .dialogue()
            .unwrap()
            .entry(dlg_tree_id, next_dlg_id)
            .unwrap()
            .text,
        "Custom Piata NPC!\nAdded by mod."
    );
    let mod_piata = validated.game_data.map(MapId(0x010)).unwrap();
    assert_eq!(mod_piata.npcs.len(), piata.npcs.len() + 1);
    assert_eq!(
        mod_piata.treasure_chests.len(),
        piata.treasure_chests.len() + 1
    );
    assert_eq!(mod_piata.warps.len(), piata.warps.len() + 1);
    let mod_battle = validated.battle_files.as_ref().unwrap();
    assert_eq!(
        mod_battle.formations.formations.len(),
        base_battle.formations.formations.len() + 1
    );
    assert_eq!(
        mod_battle.formations.formation_count as usize,
        mod_battle.formations.formations.len()
    );

    // Round-trip through strict diff_packs -> PackOverlay::load_validated.
    let edited_copy = scratch.join("edited-copy");
    tests::materialize_overlay_for_diff_test(&overlay, &edited_copy);
    let exported = scratch.join("exported");
    let summary = diff_packs(
        &base,
        &edited_copy,
        &exported,
        &ModManifest::new("real-exported", "Real Exported", "1.0.0"),
    )
    .expect("strict diff_packs must round-trip all 5 additive tables on real pack");
    assert!(summary.derived_items.is_empty());

    let (_, revalidated) =
        PackOverlay::load_validated(&base, &[load_mod(&exported).unwrap()]).unwrap();
    assert_eq!(
        revalidated.game_data.map(MapId(0x010)).unwrap().npcs.len(),
        mod_piata.npcs.len()
    );
    assert_eq!(
        revalidated
            .battle_files
            .as_ref()
            .unwrap()
            .formations
            .formations
            .len(),
        mod_battle.formations.formations.len()
    );

    let _ = fs::remove_dir_all(&scratch);
}
