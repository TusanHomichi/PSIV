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

    // Dynamic maps and declared optional fields remain patchable and removable.
    let mut map = json!({
        "id": 16,
        "treasure_chests": [{
            "index": 0,
            "contents_type": "item",
            "item_id": 125,
            "meseta": null
        }],
        "map_update_images": {}
    });
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
