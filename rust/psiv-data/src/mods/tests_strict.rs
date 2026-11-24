use super::*;
use patch::{FilePattern, KEYED_ARRAYS, array_key_field};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

static STRICT_NEXT_ID: AtomicUsize = AtomicUsize::new(1);

fn temp_dir(label: &str) -> PathBuf {
    let n = STRICT_NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "psiv-mods-strict-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn real_pack() -> Option<PathBuf> {
    let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
    pack.join("manifest.json").is_file().then_some(pack)
}

#[test]
fn mod_manifest_and_profile_reject_unknown_fields() {
    let scratch = temp_dir("manifest");
    let mod_dir = scratch.join("bad-manifest");
    fs::create_dir_all(&mod_dir).unwrap();
    fs::write(
        mod_dir.join(MOD_MANIFEST_FILE),
        r#"{
            "format_version": 1,
            "id": "bad-manifest",
            "name": "Bad Manifest",
            "version": "1.0.0",
            "enabled_flag": true
        }"#,
    )
    .unwrap();

    let err = load_mod(&mod_dir).expect_err("unknown field in mod.json must be rejected");
    assert_eq!(err.file, MOD_MANIFEST_FILE);
    assert!(err.detail.contains("unknown field"), "{err}");
    assert!(err.detail.contains("enabled_flag"), "{err}");

    let profile_dir = scratch.join("bad-profile");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(
        profile_dir.join(MOD_PROFILE_FILE),
        r#"{"format_version": 1, "mods": [], "extra_key": 1}"#,
    )
    .unwrap();
    let perr = ModProfile::read_from_dir(&profile_dir)
        .expect_err("unknown field in mod_profile.json must be rejected");
    assert_eq!(perr.file, MOD_PROFILE_FILE);
    assert!(perr.detail.contains("extra_key"), "{perr}");

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn apply_json_patch_fails_closed_on_unknown_fields_and_supports_allowed_optionals() {
    let base = json!({
        "format_version": 2,
        "count": 1,
        "enemies": [{
            "id": 10,
            "hp": 25,
            "stats": {"attack": 12, "defense": 4},
            "properties": {"physical": {"value": 2}, "fire": {"value": 2}}
        }]
    });

    // Top-level record typo, null typo, nested struct typo, and element map typo all fail.
    for (patch, expected_path) in [
        (json!({"enemies": {"10": {"HP": 99}}}), "enemies.10.HP"),
        (json!({"enemies": {"10": {"HP": null}}}), "enemies.10.HP"),
        (
            json!({"enemies": {"10": {"stats": {"ATK": 20}}}}),
            "enemies.10.stats.ATK",
        ),
        (
            json!({"enemies": {"10": {"properties": {"firee": {"value": 0}}}}}),
            "enemies.10.properties.firee",
        ),
    ] {
        let mut target = base.clone();
        let err = apply_json_patch(&mut target, &patch, "battle/enemies.json")
            .expect_err("unknown patch field must fail closed");
        assert!(err.contains("unknown field"), "{err}");
        assert!(err.contains(expected_path), "{err}");
    }

    // Dialogue shorthand rejects misplaced/invalid keep_portrait and extra unknown keys.
    let trees = json!({
        "trees": [{
            "tree": 34,
            "entries": [{
                "id": 0,
                "text": "Hi",
                "segments": [{"text": "Hi"}],
                "pages": [{"lines": ["Hi"], "end": "end"}]
            }]
        }]
    });
    for (entry_patch, needle) in [
        (
            json!({"lines": ["Yo"], "typo": 1}),
            "trees.34.entries.0.typo",
        ),
        (json!({"keep_portrait": false}), "keep_portrait"),
        (
            json!({"lines": ["Yo"], "keep_portrait": "nope"}),
            "keep_portrait",
        ),
    ] {
        let mut target = trees.clone();
        let patch = json!({"trees": {"34": {"entries": {"0": entry_patch}}}});
        let err = apply_json_patch(&mut target, &patch, "dialogue/trees.json")
            .expect_err("invalid dialogue patch must fail closed");
        assert!(err.contains(needle), "{err}");
    }

    // Dynamic maps and modeled optional fields remain patchable and removable on full MapRecord JSON.
    let mut map = minimal_valid_map_json();
    map.as_object_mut().unwrap().remove("png_over");
    map["treasure_chests"] = json!([{
        "index": 0,
        "record_offset": "0x129004",
        "x_cell": 1,
        "y_cell": 1,
        "x_pixels": 16,
        "y_pixels": 0,
        "white_chest": false,
        "object_symbol": "TreasureChest",
        "contents_type": "item",
        "item_id": 125,
        "item_symbol": "Dagger",
        "meseta": null,
        "chest_flag": 0
    }]);
    map["map_update_images"] = json!({});
    // Legacy wrong key "treasures" fails closed.
    let err = apply_json_patch(
        &mut map,
        &json!({"treasures": {"0": {"meseta": 500}}}),
        "maps/010_Piata.json",
    )
    .expect_err("wrong map key treasures must be rejected");
    assert!(err.contains("treasures"), "{err}");

    // Adding an optional field and dynamic map entry succeeds, as does nulling and re-setting a field.
    apply_json_patch(
        &mut map,
        &json!({
            "png_over": "maps/010_Piata_over.png",
            "map_update_images": {"maps/010_Piata.png": "maps/010_Piata_idx.png"},
            "treasure_chests": {"0": {"contents_type": "meseta", "item_id": null, "meseta": 500}}
        }),
        "maps/010_Piata.json",
    )
    .unwrap();
    assert_eq!(map["png_over"], "maps/010_Piata_over.png");
    assert_eq!(
        map["map_update_images"]["maps/010_Piata.png"],
        "maps/010_Piata_idx.png"
    );
    assert_eq!(map["treasure_chests"][0]["item_id"], json!(null));
    assert_eq!(map["treasure_chests"][0]["meseta"], json!(500));

    // A second mod can re-populate `item_id` after the first mod nulled it, and remove dynamic keys.
    apply_json_patch(
        &mut map,
        &json!({
            "map_update_images": {"maps/010_Piata.png": null},
            "treasure_chests": {"0": {"contents_type": "item", "item_id": 42, "meseta": null}}
        }),
        "maps/010_Piata.json",
    )
    .unwrap();
    assert!(map["map_update_images"].as_object().unwrap().is_empty());
    assert_eq!(map["treasure_chests"][0]["item_id"], json!(42));
}

#[test]
fn keyed_arrays_match_fixture_schemas_and_round_trip() {
    // Every entry in KEYED_ARRAYS must resolve via array_key_field and round-trip sparse patches.
    for spec in KEYED_ARRAYS {
        let sample_file = match spec.file {
            FilePattern::Exact(file) => file.to_owned(),
            FilePattern::Prefix { prefix, suffix } => format!("{prefix}010_Sample{suffix}"),
        };
        assert_eq!(
            array_key_field(&sample_file, spec.path),
            Some(spec.key_field),
            "array_key_field mismatch for {sample_file} {:?}",
            spec.path
        );

        let mut base = json!([
            { spec.key_field: 1, "value": 10 },
            { spec.key_field: 2, "value": 20 }
        ]);
        let mut patch_val = json!({"1": {"value": 99}});
        for seg in spec.path.iter().rev() {
            if *seg == "*" {
                let star_idx = spec.path.iter().position(|s| *s == "*").unwrap();
                let parent_path = &spec.path[..star_idx];
                let parent_key = array_key_field(&sample_file, parent_path)
                    .expect("wildcard parent must itself be a keyed array");
                let after_star = spec.path[star_idx + 1];
                let mut child_obj = serde_json::Map::new();
                child_obj.insert(parent_key.to_owned(), json!(7));
                child_obj.insert(after_star.to_owned(), base);
                base = serde_json::Value::Array(vec![serde_json::Value::Object(child_obj)]);
                patch_val = json!({"7": { after_star: patch_val }});
            } else {
                if spec.path.contains(&"*") && *seg == spec.path[spec.path.len() - 1] {
                    continue;
                }
                base = json!({ *seg: base });
                patch_val = json!({ *seg: patch_val });
            }
        }

        let mut applied = base.clone();
        apply_json_patch(&mut applied, &patch_val, &sample_file)
            .unwrap_or_else(|e| panic!("{sample_file} {:?}: {e}", spec.path));
        let round_trip = diff_json(&base, &applied, &sample_file)
            .unwrap_or_else(|e| panic!("{sample_file} {:?}: {e}", spec.path))
            .expect("edited fixture must produce a non-empty diff");
        assert_eq!(round_trip, patch_val, "{sample_file} {:?}", spec.path);
    }

    // Verify MapRecord's actual serialized field names include all five map keyed arrays
    // (guards against struct field renames even in ROM-free CI runs).
    let sample_map = crate::MapRecord {
        format_version: PACK_FORMAT_VERSION,
        id: crate::MapId(0x010),
        symbol: Some("Piata".into()),
        png: "maps/010_Piata.png".into(),
        png_over: None,
        dimensions: crate::Dimensions {
            width_cells: 2,
            height_cells: 2,
            width_chunks: 1,
            height_chunks: 1,
            width_pixels: 32,
            height_pixels: 32,
            cell_pixels: 16,
        },
        collision: crate::Collision {
            plane: crate::Plane::Fg,
            plane_byte: 0,
            grid: crate::CollisionGrid::new(2, 2, vec![crate::CollisionType::Normal; 4]).unwrap(),
        },
        scroll: crate::MapScroll::default(),
        music: crate::Music {
            id: 0,
            symbol: None,
            changes_music: false,
        },
        flags: crate::Flags {
            poison: 0,
            random_battles: 0,
            town_teleport: 0,
            dungeon_teleport_index: None,
        },
        dialogue_tree: 1,
        events: vec![0],
        warps: Vec::new(),
        interaction_areas: Vec::new(),
        npcs: Vec::new(),
        treasure_chests: Vec::new(),
        map_effects: Vec::new(),
        map_updates: Some(Vec::new()),
        map_update_palette: None,
        map_update_images: BTreeMap::new(),
        art_cycle_images: BTreeMap::new(),
        layout_variants: Vec::new(),
        vehicle_battle: None,
        patch_tiles: None,
        layout_patches: Vec::new(),
        overworld_patches: None,
        chunk_def_copies: Vec::new(),
    };
    let map_val = serde_json::to_value(&sample_map).unwrap();
    for spec in KEYED_ARRAYS
        .iter()
        .filter(|s| s.file.matches("maps/010_Piata.json"))
    {
        let field = spec.path[0];
        assert!(
            map_val
                .get(field)
                .and_then(serde_json::Value::as_array)
                .is_some(),
            "MapRecord JSON must carry array field `{field}`"
        );
    }
}

#[test]
fn keyed_arrays_match_real_runtime_pack() {
    let Some(base) = real_pack() else {
        eprintln!("runtime-pack absent; skipping real-pack KEYED_ARRAYS guard test");
        return;
    };

    let all_files = collect_relative_files(&base, "guard", "base-pack").unwrap();
    let rel_files: Vec<String> = all_files
        .iter()
        .map(|rel| normalize_rel(rel, "guard", "base-pack").unwrap())
        .collect();

    fn walk_arrays<'a>(
        value: &'a serde_json::Value,
        path: &[&str],
        out: &mut Vec<&'a [serde_json::Value]>,
    ) -> Result<(), String> {
        let Some((head, tail)) = path.split_first() else {
            let arr = value
                .as_array()
                .ok_or_else(|| "expected JSON array at keyed path".to_owned())?;
            out.push(arr.as_slice());
            return Ok(());
        };
        if *head == "*" {
            let arr = value
                .as_array()
                .ok_or_else(|| "expected JSON array at `*` segment".to_owned())?;
            for item in arr {
                walk_arrays(item, tail, out)?;
            }
            return Ok(());
        }
        let next = value
            .as_object()
            .and_then(|obj| obj.get(*head))
            .ok_or_else(|| format!("missing field `{head}`"))?;
        walk_arrays(next, tail, out)
    }

    for spec in KEYED_ARRAYS {
        let matching: Vec<&str> = rel_files
            .iter()
            .map(String::as_str)
            .filter(|rel| spec.file.matches(rel))
            .collect();
        assert!(
            !matching.is_empty(),
            "KEYED_ARRAYS spec {:?} matched no files in runtime-pack",
            spec
        );

        let mut total_items = 0usize;
        for rel in matching {
            let text = fs::read_to_string(base.join(rel)).unwrap();
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            let mut arrays = Vec::new();
            walk_arrays(&json, spec.path, &mut arrays)
                .unwrap_or_else(|e| panic!("{rel} {:?}: {e}", spec.path));
            for arr in arrays {
                let mut seen = std::collections::BTreeSet::new();
                for (idx, item) in arr.iter().enumerate() {
                    let key_val = item
                        .as_object()
                        .and_then(|obj| obj.get(spec.key_field))
                        .unwrap_or_else(|| {
                            panic!(
                                "{rel} {:?}[{idx}]: missing key field `{}`",
                                spec.path, spec.key_field
                            )
                        });
                    let key_str = match key_val {
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::String(s) => s.clone(),
                        other => panic!(
                            "{rel} {:?}[{idx}]: key field `{}` has unsupported type {other}",
                            spec.path, spec.key_field
                        ),
                    };
                    assert!(
                        seen.insert(key_str.clone()),
                        "{rel} {:?}: duplicate {}={key_str}",
                        spec.path,
                        spec.key_field
                    );
                    total_items += 1;
                }
            }
        }
        assert!(
            total_items > 0,
            "KEYED_ARRAYS spec {:?} matched zero records in runtime-pack",
            spec
        );
    }
}

#[test]
fn schema_driven_validation_allows_omitted_optionals_and_rejects_nested_typos() {
    // 1. Modeled optional field omitted from base (`Ability.symbol`) round-trips through diff -> apply,
    // while a misspelled key (`symbl`) fails closed.
    let base_abilities = json!({
        "format_version": 2,
        "effects": {
            "table": "0x0061BE",
            "count": 44,
            "bounds_checked": false
        },
        "techniques": [{
            "id": 0,
            "kind": "techniques",
            "name": "Foi",
            "effect_id": 0,
            "tp_cost": 2,
            "targeting": {"raw": 0},
            "effect_out_of_range": false
        }],
        "skills": [],
        "enemy_skills": [],
        "item_effects": [],
        "combos": []
    });
    assert!(
        serde_json::from_value::<crate::battle::AbilitiesFile>(base_abilities.clone()).is_ok(),
        "test fixture must parse as AbilitiesFile"
    );
    let mut edited_abilities = base_abilities.clone();
    edited_abilities["techniques"][0]["symbol"] = json!("Tech_Foi");
    let patch = diff_json(&base_abilities, &edited_abilities, "battle/abilities.json")
        .unwrap()
        .expect("adding optional Ability.symbol produces a diff");
    let mut patched_abilities = base_abilities.clone();
    apply_json_patch(&mut patched_abilities, &patch, "battle/abilities.json")
        .expect("modeled optional Ability.symbol omitted from base must be accepted");
    assert_eq!(patched_abilities["techniques"][0]["symbol"], "Tech_Foi");

    let mut bad_abilities = base_abilities;
    let err = apply_json_patch(
        &mut bad_abilities,
        &json!({"techniques": {"0": {"symbl": "Tech_Foi"}}}),
        "battle/abilities.json",
    )
    .expect_err("misspelled optional field must fail closed");
    assert!(err.contains("symbl"), "{err}");

    // 2. Legacy map JSON omitting `interaction_areas`, replacing `patch_tiles: null`,
    // and replacing non-null unkeyed `map_effects` all fail closed on typos.
    let base_map = minimal_valid_map_json();
    let mut legacy_map = base_map.clone();
    legacy_map
        .as_object_mut()
        .unwrap()
        .remove("interaction_areas");
    let err = apply_json_patch(
        &mut legacy_map,
        &json!({
            "interaction_areas": [{
                "index": 0,
                "record_offset": "0x0",
                "range": {"id": 0, "name": "XYExact"},
                "source": {"x_byte": 0, "y_byte": 0, "x_cell": 0, "y_cell": 0},
                "rect": null,
                "flag_type": {"id": 0, "name": null},
                "flag": 0,
                "interaction_type": 0,
                "parameter": 0,
                "typo_field": 1
            }]
        }),
        "maps/010_Piata.json",
    )
    .expect_err("typo inside newly inserted interaction_areas must fail");
    assert!(err.contains("typo_field"), "{err}");

    let mut null_patch_tiles_map = base_map.clone();
    null_patch_tiles_map["patch_tiles"] = json!(null);
    let err = apply_json_patch(
        &mut null_patch_tiles_map,
        &json!({
            "patch_tiles": {
                "png": "maps/010_patch.png",
                "tile_pixels": 32,
                "count": 0,
                "tiles": [],
                "extra_typo": true
            }
        }),
        "maps/010_Piata.json",
    )
    .expect_err("typo inside null-replaced patch_tiles must fail");
    assert!(err.contains("patch_tiles.extra_typo"), "{err}");

    // Non-null unkeyed array `map_effects` replaced wholesale must also reject typos.
    let mut non_null_effects_map = base_map.clone();
    non_null_effects_map["map_effects"] = json!([{"entry": 1, "decoded": true}]);
    let err = apply_json_patch(
        &mut non_null_effects_map,
        &json!({"map_effects": [{"entry": 1, "decoded": true, "bogus_effect_key": 7}]}),
        "maps/010_Piata.json",
    )
    .expect_err("typo inside replaced non-null map_effects must fail");
    assert!(err.contains("map_effects[0].bogus_effect_key"), "{err}");

    // 2b. Null-valued unknown keys on a schema-complete file must fail closed
    // instead of being silently dropped before schema_check runs.
    let mut null_unknown_top = base_map.clone();
    let err = apply_json_patch(
        &mut null_unknown_top,
        &json!({"HP": null}),
        "maps/010_Piata.json",
    )
    .expect_err("null-valued unknown top-level key must fail closed");
    assert!(err.contains("unknown field"), "{err}");
    assert!(err.contains("HP"), "{err}");

    let mut null_unknown_nested = base_map.clone();
    let err = apply_json_patch(
        &mut null_unknown_nested,
        &json!({"music": {"typo": null}}),
        "maps/010_Piata.json",
    )
    .expect_err("null-valued unknown nested key must fail closed");
    assert!(err.contains("unknown field"), "{err}");
    assert!(err.contains("music.typo"), "{err}");

    // Positive control: known Option<T> fields and dynamic map deletions still accept null.
    let mut null_valid = base_map;
    null_valid["map_update_images"] = json!({"maps/010_Piata.png": "maps/010_Piata_indices.png"});
    apply_json_patch(
        &mut null_valid,
        &json!({
            "png_over": null,
            "patch_tiles": null,
            "map_update_images": {"maps/010_Piata.png": null}
        }),
        "maps/010_Piata.json",
    )
    .unwrap();
    assert!(
        !null_valid["map_update_images"]
            .as_object()
            .unwrap()
            .contains_key("maps/010_Piata.png")
    );

    // 3. Legacy `dialogue/trees.json` omitting `system_messages` rejects typos inside nested `DialogueEntry`.
    let mut legacy_trees = json!({
        "format_version": 2,
        "tree_count": 1,
        "entry_count": 1,
        "window": {
            "chars_per_line": 32,
            "lines_per_window": 2,
            "glyph_width": 8,
            "glyph_height": 16,
            "rect": {"x": 32, "y": 160, "width": 256, "height": 32},
            "scroll_arrow": {"screen_x": 280, "screen_y": 192, "png": "dialogue/arrow.png", "width": 16, "height": 8}
        },
        "trees": [{
            "tree": 1,
            "label": "DialogueTree1",
            "is_talk_tree": false,
            "portrait_operand_bytes": 1,
            "entry_count": 1,
            "entries": [{
                "id": 0,
                "segments": [{"text": "Hi"}],
                "pages": [{"lines": ["Hi"], "end": "end"}],
                "text": "Hi"
            }]
        }]
    });
    assert!(
        serde_json::from_value::<crate::dialogue::TreeFile>(legacy_trees.clone()).is_ok(),
        "test fixture must parse as TreeFile"
    );
    let err = apply_json_patch(
        &mut legacy_trees,
        &json!({
            "system_messages": {
                "messages": [{
                    "id": 0,
                    "segments": [{"text": "Sys"}],
                    "pages": [{"lines": ["Sys"], "end": "end"}],
                    "text": "Sys",
                    "typo": 1
                }]
            }
        }),
        "dialogue/trees.json",
    )
    .expect_err("typo inside newly inserted system_messages.messages[0] must fail");
    assert!(err.contains("system_messages.messages[0].typo"), "{err}");
}

fn minimal_valid_map_json() -> serde_json::Value {
    let val = json!({
        "format_version": 2,
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
    });
    assert!(
        serde_json::from_value::<crate::MapRecord>(val.clone()).is_ok(),
        "test fixture must parse as MapRecord"
    );
    val
}

#[test]
fn real_pack_strict_fields_and_treasure_chests_round_trip() {
    let Some(base) = real_pack() else {
        eprintln!("runtime-pack absent; skipping real-pack strict fields test");
        return;
    };

    let scratch = temp_dir("real-strict");

    // Negative control 1: typo field ("HP": 99) fails closed naming mod, file, and field.
    let bad_typo = scratch.join("bad-typo");
    init_mod_dir(
        &bad_typo,
        &ModManifest::new("bad-typo", "Bad Typo", "1.0.0"),
    )
    .unwrap();
    fs::create_dir_all(bad_typo.join("patches/battle")).unwrap();
    fs::write(
        bad_typo.join("patches/battle/enemies.json"),
        r#"{"enemies":{"10":{"HP":99}}}"#,
    )
    .unwrap();
    let err = apply_mods(
        &base,
        &[load_mod(&bad_typo).unwrap()],
        &scratch.join("bad-typo-out"),
    )
    .expect_err("typo field HP must be refused");
    assert_eq!(err.mod_id, "bad-typo");
    assert!(err.file.contains("battle/enemies.json"), "{err}");
    assert!(
        err.detail.contains("enemies[10].HP") || err.detail.contains("enemies.10.HP"),
        "{err}"
    );

    // Negative control 1b: null-valued typo field ("HP": null) also fails closed on real pack.
    fs::write(
        bad_typo.join("patches/battle/enemies.json"),
        r#"{"enemies":{"10":{"HP":null}}}"#,
    )
    .unwrap();
    let null_err = apply_mods(
        &base,
        &[load_mod(&bad_typo).unwrap()],
        &scratch.join("bad-typo-null-out"),
    )
    .expect_err("null-valued typo field HP must be refused");
    assert_eq!(null_err.mod_id, "bad-typo");
    assert!(null_err.file.contains("battle/enemies.json"), "{null_err}");
    assert!(null_err.detail.contains("HP"), "{null_err}");

    // Negative control 2: legacy wrong map key ("treasures") fails closed.
    let bad_map_key = scratch.join("bad-map-key");
    init_mod_dir(
        &bad_map_key,
        &ModManifest::new("bad-map-key", "Bad Map Key", "1.0.0"),
    )
    .unwrap();
    fs::create_dir_all(bad_map_key.join("patches/maps")).unwrap();
    fs::write(
        bad_map_key.join("patches/maps/010_Piata.json"),
        r#"{"treasures":{"0":{"meseta":500}}}"#,
    )
    .unwrap();
    let err = apply_mods(
        &base,
        &[load_mod(&bad_map_key).unwrap()],
        &scratch.join("bad-map-key-out"),
    )
    .expect_err("unknown map key treasures must be refused");
    assert_eq!(err.mod_id, "bad-map-key");
    assert!(err.file.contains("maps/010_Piata.json"), "{err}");
    assert!(err.detail.contains("treasures"), "{err}");

    // Positive + round-trip control: sparse treasure_chests patch by `index` on a real map.
    let base_game = GameData::load(&base).unwrap();
    let (chest_map_id, chest_map) = base_game
        .maps()
        .find(|(_, m)| !m.treasure_chests.is_empty())
        .expect("retail pack has maps with treasure chests");
    let chest_map_rel = base_game
        .manifest()
        .entry(chest_map_id)
        .unwrap()
        .json
        .clone();
    let orig_chest = &chest_map.treasure_chests[0];
    let toggled_white = !orig_chest.white_chest;
    let chest_mod = scratch.join("chest-mod");
    init_mod_dir(
        &chest_mod,
        &ModManifest::new("chest-mod", "Chest Mod", "1.0.0"),
    )
    .unwrap();
    let chest_patch_path = chest_mod.join("patches").join(&chest_map_rel);
    fs::create_dir_all(chest_patch_path.parent().unwrap()).unwrap();
    fs::write(
        &chest_patch_path,
        serde_json::to_string_pretty(&json!({
            "treasure_chests": {
                orig_chest.index.to_string(): {
                    "white_chest": toggled_white,
                    "contents_type": "meseta",
                    "item_id": null,
                    "item_symbol": null,
                    "meseta": 777
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let chest_applied = apply_mods(
        &base,
        &[load_mod(&chest_mod).unwrap()],
        &scratch.join("chest-out"),
    )
    .unwrap();
    let chest_game = GameData::load(&chest_applied.effective_pack).unwrap();
    let patched_chest = &chest_game.map(chest_map_id).unwrap().treasure_chests[0];
    assert_eq!(patched_chest.white_chest, toggled_white);
    assert_eq!(patched_chest.contents_type, crate::ContentsType::Meseta);
    assert_eq!(patched_chest.item_id, None);
    assert_eq!(patched_chest.meseta, Some(777));

    let chest_exported = scratch.join("chest-exported");
    let chest_summary = diff_packs(
        &base,
        &chest_applied.effective_pack,
        &chest_exported,
        &ModManifest::new("chest-exported", "Chest Exported", "1.0.0"),
    )
    .unwrap();
    assert_eq!(chest_summary.patches, vec![chest_map_rel.clone()]);
    let exported_patch_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(chest_exported.join("patches").join(&chest_map_rel)).unwrap(),
    )
    .unwrap();
    assert!(
        exported_patch_json["treasure_chests"]
            .get(orig_chest.index.to_string())
            .is_some(),
        "diff_packs must emit sparse index-keyed treasure_chests patch: {exported_patch_json}"
    );
    let chest_reapplied = apply_mods(
        &base,
        &[load_mod(&chest_exported).unwrap()],
        &scratch.join("chest-reapplied"),
    )
    .unwrap();
    let chest_reapplied_game = GameData::load(&chest_reapplied.effective_pack).unwrap();
    let re_chest = &chest_reapplied_game
        .map(chest_map_id)
        .unwrap()
        .treasure_chests[0];
    assert_eq!(re_chest.white_chest, toggled_white);
    assert_eq!(re_chest.meseta, Some(777));

    // Negative control 3: unknown field inside a `null`-replacing SpriteRef object fails closed.
    if let Some((null_npc_map_id, null_npc_map)) = base_game
        .maps()
        .find(|(_, m)| m.npcs.iter().any(|n| n.sprite.is_none()))
    {
        let null_npc = null_npc_map
            .npcs
            .iter()
            .find(|n| n.sprite.is_none())
            .unwrap();
        let map_rel = base_game
            .manifest()
            .entry(null_npc_map_id)
            .unwrap()
            .json
            .clone();
        let bad_sprite_mod = scratch.join("bad-sprite");
        init_mod_dir(
            &bad_sprite_mod,
            &ModManifest::new("bad-sprite", "Bad Sprite", "1.0.0"),
        )
        .unwrap();
        let patch_path = bad_sprite_mod.join("patches").join(&map_rel);
        fs::create_dir_all(patch_path.parent().unwrap()).unwrap();
        fs::write(
            &patch_path,
            serde_json::to_string_pretty(&json!({
                "npcs": {
                    null_npc.index.to_string(): {
                        "sprite": {
                            "sheet": "Chaz",
                            "sheets": "sprites/party.json",
                            "idle_sequence": "idle_down",
                            "walk_sequence": "walk_down",
                            "typo_field": true
                        }
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let err = apply_mods(
            &base,
            &[load_mod(&bad_sprite_mod).unwrap()],
            &scratch.join("bad-sprite-out"),
        )
        .expect_err("unknown field in null-replacing SpriteRef must be refused");
        assert_eq!(err.mod_id, "bad-sprite");
        assert!(err.file.contains(&map_rel), "{err}");
        assert!(err.detail.contains("typo_field"), "{err}");
    }

    let _ = fs::remove_dir_all(&scratch);
}
