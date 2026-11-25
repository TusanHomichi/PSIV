//! Tests for sparse cell-level grid patches, expanded keyed arrays, dialogue
//! shorthand diffing, strict fail-closed shareability, and shareability audit
//! (#789).

use super::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

static SHARE_NEXT_ID: AtomicUsize = AtomicUsize::new(1);

fn temp_dir(label: &str) -> PathBuf {
    let n = SHARE_NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "psiv-mods-share-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
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

fn sample_map_json() -> Value {
    json!({
        "format_version": PACK_FORMAT_VERSION,
        "id": 16,
        "symbol": "Piata",
        "png": "maps/010_Piata.png",
        "png_over": null,
        "dimensions": {
            "width_chunks": 2,
            "height_chunks": 2,
            "width_cells": 4,
            "height_cells": 4,
            "width_pixels": 64,
            "height_pixels": 64,
            "cell_pixels": 16
        },
        "collision": {
            "plane": "bg",
            "plane_byte": 1,
            "width_cells": 4,
            "height_cells": 4,
            "rows": [
                [0, 0, 0, 8],
                [0, 1, 0, 8],
                [0, 0, 2, 8],
                [8, 8, 8, 8]
            ]
        },
        "vehicle_battle": {
            "plane": "bg",
            "width_chunks": 2,
            "height_chunks": 2,
            "rows": [[1, 2], [3, 4]]
        },
        "layout_variants": [{
            "png": "maps/010_Piata_variant.png",
            "collision": {
                "plane": "bg",
                "width_cells": 2,
                "height_cells": 2,
                "rows": [[0, 8], [8, 0]]
            },
            "vehicle_battle": {
                "plane": "bg",
                "width_chunks": 1,
                "height_chunks": 1,
                "rows": [[10]]
            }
        }],
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

#[test]
fn sparse_cell_grid_patch_and_diff_round_trip_for_maps_and_formations() {
    let base = sample_map_json();
    let mut edited = base.clone();
    edited["collision"]["rows"][1][2] = json!(8);
    edited["collision"]["rows"][3][0] = json!(0);
    edited["vehicle_battle"]["rows"][0][1] = json!(512);
    edited["layout_variants"][0]["collision"]["rows"][0][0] = json!(9);
    edited["layout_variants"][0]["vehicle_battle"]["rows"][0][0] = json!(42);

    let patch = diff_json(&base, &edited, "maps/010_Piata.json")
        .expect("strict diff_json must emit sparse cell deltas")
        .expect("non-empty diff");
    assert_eq!(
        patch,
        json!({
            "collision": {
                "rows": {
                    "cells": [
                        [1, 2, 8],
                        [3, 0, 0]
                    ]
                }
            },
            "vehicle_battle": {
                "rows": {
                    "cells": [
                        [0, 1, 512]
                    ]
                }
            },
            "layout_variants": {
                "maps/010_Piata_variant.png": {
                    "collision": {
                        "rows": {
                            "cells": [[0, 0, 9]]
                        }
                    },
                    "vehicle_battle": {
                        "rows": {
                            "cells": [[0, 0, 42]]
                        }
                    }
                }
            }
        })
    );

    let mut applied = base.clone();
    apply_json_patch(&mut applied, &patch, "maps/010_Piata.json").unwrap();
    assert_eq!(applied, edited);
    let record: crate::MapRecord = serde_json::from_value(applied).unwrap();
    assert_eq!(
        record.collision.grid.at(2, 1),
        Some(crate::CollisionType::Solid)
    );
    assert_eq!(
        record.collision.grid.at(0, 3),
        Some(crate::CollisionType::Normal)
    );
    assert_eq!(record.vehicle_battle.as_ref().unwrap().rows[0][1], 512);
    assert_eq!(record.layout_variants[0].collision.rows[0][0], 9);

    // Explicit object form `{"row", "col", "value"}` and hex-nibble base rows also work.
    let mut hex_base = base.clone();
    hex_base["collision"]["rows"] = json!(["0008", "0108", "0028", "8888"]);
    let obj_patch = json!({
        "collision": {
            "rows": {
                "cells": [
                    {"row": 1, "col": 2, "value": 12},
                    [3, 0, 0]
                ]
            }
        }
    });
    apply_json_patch(&mut hex_base, &obj_patch, "maps/010_Piata.json").unwrap();
    assert_eq!(
        hex_base["collision"]["rows"],
        json!(["0008", "01c8", "0028", "0888"])
    );
    let hex_diff = diff_json(&base, &hex_base, "maps/010_Piata.json")
        .unwrap()
        .expect("hex diff");
    assert_eq!(
        hex_diff,
        json!({"collision": {"rows": {"cells": [[1, 2, 12], [3, 0, 0]]}}})
    );

    // Overworld battle position_grids.*.cells sparse cell patch/diff.
    let base_form = json!({
        "position_grids": [{
            "name": "motavia",
            "columns": 2,
            "rows": 2,
            "cells": [[0, 1], [2, 255]]
        }]
    });
    let mut edited_form = base_form.clone();
    edited_form["position_grids"][0]["cells"][1][0] = json!(7);
    let form_patch = diff_json(&base_form, &edited_form, "battle/formations.json")
        .unwrap()
        .expect("position grid diff");
    assert_eq!(
        form_patch,
        json!({"position_grids": {"motavia": {"cells": {"cells": [[1, 0, 7]]}}}})
    );
    let mut applied_form = base_form;
    apply_json_patch(&mut applied_form, &form_patch, "battle/formations.json").unwrap();
    assert_eq!(applied_form, edited_form);
}

#[test]
fn sparse_cell_grid_patch_rejects_invalid_bounds_types_duplicates_and_unknown_fields() {
    let base = sample_map_json();
    let cases = [
        (
            json!({"collision": {"rows": {"cells": [[4, 0, 8]]}}}),
            "out of bounds",
        ),
        (
            json!({"collision": {"rows": {"cells": [[0, 4, 8]]}}}),
            "out of bounds",
        ),
        (
            json!({"collision": {"rows": {"cells": [[0, 0, 16]]}}}),
            "out of range",
        ),
        (
            json!({"collision": {"rows": {"cells": [[-1, 0, 8]]}}}),
            "non-negative integer",
        ),
        (
            json!({"collision": {"rows": {"cells": [[0, "1", 8]]}}}),
            "non-negative integer",
        ),
        (
            json!({"collision": {"rows": {"cells": [[0, 0, "8"]]}}}),
            "4-bit collision code",
        ),
        (
            json!({"collision": {"rows": {"cells": [[0, 0]]}}}),
            "3-element",
        ),
        (
            json!({"collision": {"rows": {"cells": [[1, 1, 8], [1, 1, 0]]}}}),
            "duplicate cell target",
        ),
        (
            json!({"collision": {"rows": {"cells": [[1, 1, 8], {"row": 1, "col": 1, "value": 0}]}}}),
            "duplicate cell target",
        ),
        (
            json!({"collision": {"rows": {"cells": [], "typo": true}}}),
            "unknown field `collision.rows.typo`",
        ),
        (
            json!({"collision": {"rows": {"cells": [{"row": 0, "col": 0, "value": 8, "x": 1}]}}}),
            "unknown field `collision.rows.cells[0].x`",
        ),
        (
            json!({"collision": {"rows": {"cells": [{"row": 0, "value": 8}]}}}),
            "missing required field `col`",
        ),
        (
            json!({"collision": {"rows": "not-a-grid"}}),
            "expected sparse cell patch",
        ),
        (
            json!({"collision": {"rows": {"cells": [[0, 0, 8]]}, "bogus_sibling": 1}}),
            "unknown field `collision.bogus_sibling`",
        ),
        (
            json!({"vehicle_battle": {"rows": {"cells": [[0, 0, 70000]]}}}),
            "out of range",
        ),
    ];

    for (patch, expected) in cases {
        let mut target = base.clone();
        let err = apply_json_patch(&mut target, &patch, "maps/010_Piata.json")
            .expect_err(&format!("patch {patch} must fail closed"));
        assert!(
            err.contains(expected),
            "expected `{expected}` in error `{err}` for patch {patch}"
        );
    }
}

#[test]
fn dialogue_diff_synthesizes_shorthand_and_refuses_unkeyed_segment_dump_by_default() {
    let base = json!({
        "trees": [{
            "tree": 34,
            "entries": [{
                "id": 0,
                "text": "Professor!\nThank goodness you're safe!",
                "segments": [
                    {"code": "0xF4", "ctrl": "portrait", "operands": [2]},
                    {"text": "Professor!"},
                    {"code": "0xFC", "ctrl": "newline", "operands": []},
                    {"text": "Thank goodness you're safe!"}
                ],
                "pages": [{
                    "lines": ["Professor!", "Thank goodness you're safe!"],
                    "end": "end"
                }]
            }]
        }]
    });

    // 1. Editing text produces a compact `replace` shorthand with zero retail array copy.
    let mut edited_replace = base.clone();
    edited_replace["trees"][0]["entries"][0]["text"] = json!("Professor!\nGlad to see you!");
    edited_replace["trees"][0]["entries"][0]["segments"][3]["text"] = json!("Glad to see you!");
    edited_replace["trees"][0]["entries"][0]["pages"][0]["lines"][1] = json!("Glad to see you!");
    let patch = diff_json(&base, &edited_replace, "dialogue/trees.json")
        .unwrap()
        .expect("non-empty diff");
    assert_eq!(
        patch,
        json!({
            "trees": {
                "34": {
                    "entries": {
                        "0": {
                            "replace": [["Thank goodness you're safe!", "Glad to see you!"]]
                        }
                    }
                }
            }
        })
    );
    let mut applied = base.clone();
    apply_json_patch(&mut applied, &patch, "dialogue/trees.json").unwrap();
    assert_eq!(applied, edited_replace);

    // 2. Rewriting pages produces a `lines` shorthand (and `keep_portrait: false` if dropped).
    let mut edited_lines = base.clone();
    apply_json_patch(
        &mut edited_lines,
        &json!({"trees": {"34": {"entries": {"0": {"lines": ["Line A", "Line B"], "keep_portrait": false}}}}}),
        "dialogue/trees.json",
    )
    .unwrap();
    let lines_diff = diff_json(&base, &edited_lines, "dialogue/trees.json")
        .unwrap()
        .expect("lines diff");
    assert_eq!(
        lines_diff,
        json!({"trees": {"34": {"entries": {"0": {"lines": ["Line A", "Line B"], "keep_portrait": false}}}}})
    );

    // 3. Injecting a custom control code that neither shorthand models is refused by default,
    //    and allowed with `allow_derived: true`.
    let mut custom_ctrl = base.clone();
    custom_ctrl["trees"][0]["entries"][0]["segments"]
        .as_array_mut()
        .unwrap()
        .push(json!({"code": "0xF5", "ctrl": "pause", "operands": [4]}));
    let err = diff_json(&base, &custom_ctrl, "dialogue/trees.json")
        .expect_err("raw unkeyed segments array dump must be refused by default");
    assert!(err.contains("segments"), "{err}");
    assert!(err.contains("--allow-derived"), "{err}");
    let allowed = diff_json_with_options(
        &base,
        &custom_ctrl,
        "dialogue/trees.json",
        DiffOptions {
            allow_derived: true,
        },
    )
    .unwrap()
    .expect("allow_derived permits unkeyed segments dump");
    let mut reapplied = base;
    apply_json_patch(&mut reapplied, &allowed, "dialogue/trees.json").unwrap();
    assert_eq!(reapplied, custom_ctrl);
}

#[test]
fn diff_packs_enforces_strict_shareability_by_default_and_allows_opt_in() {
    let scratch = temp_dir("strict-shareability");
    let base = scratch.join("base");
    let edited = scratch.join("edited");
    let out_mod = scratch.join("mod");

    fs::create_dir_all(base.join("maps")).unwrap();
    fs::write(base.join("manifest.json"), "{}\n").unwrap();
    fs::write(
        base.join("maps/010_Piata.json"),
        serde_json::to_string_pretty(&sample_map_json()).unwrap(),
    )
    .unwrap();
    fs::write(base.join("maps/010_Piata.png"), b"retail-png-bytes").unwrap();

    fs::create_dir_all(edited.join("maps")).unwrap();
    fs::write(edited.join("manifest.json"), "{}\n").unwrap();
    fs::write(edited.join("maps/010_Piata.png"), b"retail-png-bytes").unwrap();

    // Seed an existing mod file to verify a refused diff never wipes prior output.
    fs::create_dir_all(out_mod.join("patches")).unwrap();
    fs::write(out_mod.join("patches/sentinel.json"), "{}").unwrap();

    let manifest = ModManifest::new("share-test", "Share Test", "1.0.0");

    // 1. Editing an unkeyed base array (`events`) is refused by default.
    let mut bad_events = sample_map_json();
    bad_events["events"] = json!([0, 99]);
    fs::write(
        edited.join("maps/010_Piata.json"),
        serde_json::to_string_pretty(&bad_events).unwrap(),
    )
    .unwrap();
    let err = diff_packs(&base, &edited, &out_mod, &manifest)
        .expect_err("unkeyed base array copy must fail closed");
    assert_eq!(err.file, "maps/010_Piata.json");
    assert!(err.detail.contains("events"), "{err}");
    assert!(err.detail.contains("--allow-derived"), "{err}");
    assert!(
        out_mod.join("patches/sentinel.json").is_file(),
        "refused diff must never wipe an existing out_mod"
    );

    // Resizing a 2D grid is also refused by default.
    let mut resized_grid = sample_map_json();
    resized_grid["collision"]["rows"] = json!([[0, 0], [0, 0]]);
    fs::write(
        edited.join("maps/010_Piata.json"),
        serde_json::to_string_pretty(&resized_grid).unwrap(),
    )
    .unwrap();
    let grid_err = diff_packs(&base, &edited, &out_mod, &manifest)
        .expect_err("resized 2D grid copy must fail closed");
    assert!(grid_err.detail.contains("collision.rows"), "{grid_err}");
    assert!(grid_err.detail.contains("--allow-derived"), "{grid_err}");

    // 2. Modifying an existing base binary asset is refused by default.
    fs::write(
        edited.join("maps/010_Piata.json"),
        serde_json::to_string_pretty(&sample_map_json()).unwrap(),
    )
    .unwrap();
    fs::write(edited.join("maps/010_Piata.png"), b"recolored-png-bytes").unwrap();
    let bin_err = diff_packs(&base, &edited, &out_mod, &manifest)
        .expect_err("modified base-path binary asset must fail closed");
    assert_eq!(bin_err.file, "maps/010_Piata.png");
    assert!(bin_err.detail.contains("--allow-derived"), "{bin_err}");

    // 3. Opting into `allow_derived: true` exports both unkeyed arrays and modified base binaries,
    //    recording every non-delta item in `derived_items`.
    fs::write(
        edited.join("maps/010_Piata.json"),
        serde_json::to_string_pretty(&bad_events).unwrap(),
    )
    .unwrap();
    let allowed = diff_packs_with_options(
        &base,
        &edited,
        &out_mod,
        &manifest,
        DiffOptions {
            allow_derived: true,
        },
    )
    .unwrap();
    assert_eq!(allowed.patches, vec!["maps/010_Piata.json"]);
    assert_eq!(allowed.assets, vec!["maps/010_Piata.png"]);
    assert_eq!(
        allowed.derived_items,
        vec![
            "patches/maps/010_Piata.json#events",
            "assets/maps/010_Piata.png"
        ]
    );

    // 4. Default strict mode ALLOWS sparse cell edits + brand-new assets not in base_pack.
    let mut sparse_cell = sample_map_json();
    sparse_cell["collision"]["rows"][0][1] = json!(8);
    fs::write(
        edited.join("maps/010_Piata.json"),
        serde_json::to_string_pretty(&sparse_cell).unwrap(),
    )
    .unwrap();
    fs::write(edited.join("maps/010_Piata.png"), b"retail-png-bytes").unwrap();
    fs::write(edited.join("maps/original_banner.png"), b"brand-new-art").unwrap();

    let clean = diff_packs(&base, &edited, &out_mod, &manifest).unwrap();
    assert_eq!(clean.patches, vec!["maps/010_Piata.json"]);
    assert_eq!(clean.assets, vec!["maps/original_banner.png"]);
    assert!(clean.derived_items.is_empty());

    // 5. `audit_mod_shareability` reports zero notices for the clean mod, a non-warning note
    //    for a hand-drawn replacement asset, a warning for an untouched base asset copy, and
    //    a whole-array notice for a raw `collision.rows` dump.
    let clean_loaded = load_mod(&out_mod).unwrap();
    assert!(
        audit_mod_shareability(&base, &clean_loaded)
            .unwrap()
            .is_empty()
    );

    fs::write(out_mod.join("assets/maps/010_Piata.png"), b"hand-drawn-art").unwrap();
    let notices = audit_mod_shareability(&base, &clean_loaded).unwrap();
    assert_eq!(
        notices,
        vec![ShareabilityNotice::ReplacedBaseAsset {
            path: "maps/010_Piata.png".into()
        }]
    );
    assert!(!notices[0].is_warning());

    fs::write(
        out_mod.join("assets/maps/010_Piata.png"),
        b"retail-png-bytes",
    )
    .unwrap();
    fs::write(
        out_mod.join("patches/maps/010_Piata.json"),
        serde_json::to_string(&json!({
            "collision": {"rows": [[0, 8, 0, 8], [0, 1, 0, 8], [0, 0, 2, 8], [8, 8, 8, 8]]},
            "events": [0, 99]
        }))
        .unwrap(),
    )
    .unwrap();
    let warn_notices = audit_mod_shareability(&base, &clean_loaded).unwrap();
    assert!(
        warn_notices.contains(&ShareabilityNotice::IdenticalBaseAsset {
            path: "maps/010_Piata.png".into()
        })
    );
    assert!(warn_notices.contains(&ShareabilityNotice::WholeBaseArray {
        file: "maps/010_Piata.json".into(),
        field: "collision.rows".into()
    }));
    assert!(warn_notices.contains(&ShareabilityNotice::WholeBaseArray {
        file: "maps/010_Piata.json".into(),
        field: "events".into()
    }));

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn real_pack_single_cell_collision_and_keyed_map_arrays_round_trip_strict() {
    let Some(base) = real_pack() else {
        eprintln!("runtime-pack absent; skipping real-pack sparse cell round-trip test");
        return;
    };

    let scratch = temp_dir("real-sparse-cell");
    let base_game = GameData::load(&base).unwrap();
    let piata = base_game.map(crate::MapId(0x010)).unwrap();
    let orig_cell = piata.collision.grid.at(1, 1).unwrap();
    let new_code = if orig_cell.code() == 8 { 0 } else { 8 };

    // Find a real map with interaction_areas and a map with map_updates.
    let (ia_map_id, ia_map) = base_game
        .maps()
        .find(|(_, m)| !m.interaction_areas.is_empty())
        .expect("retail pack has maps with interaction_areas");
    let ia_rel = base_game.manifest().entry(ia_map_id).unwrap().json.clone();
    let ia = &ia_map.interaction_areas[0];

    let mod_dir = scratch.join("mods/sparse-map");
    init_mod_dir(
        &mod_dir,
        &ModManifest::new("sparse-map", "Sparse Map", "1.0.0"),
    )
    .unwrap();
    fs::create_dir_all(mod_dir.join("patches/maps")).unwrap();
    fs::write(
        mod_dir.join("patches/maps/010_Piata.json"),
        serde_json::to_string_pretty(&json!({
            "collision": {
                "rows": {
                    "cells": [[1, 1, new_code]]
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
    if ia_rel != "maps/010_Piata.json" {
        fs::write(
            mod_dir.join("patches").join(&ia_rel),
            serde_json::to_string_pretty(&json!({
                "interaction_areas": {
                    ia.index.to_string(): {
                        "parameter": ia.parameter.wrapping_add(1)
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
    }

    let applied = apply_mods(
        &base,
        &[load_mod(&mod_dir).unwrap()],
        &scratch.join("effective"),
    )
    .unwrap();
    let loaded_game = GameData::load(&applied.effective_pack).unwrap();
    assert_eq!(
        loaded_game
            .map(crate::MapId(0x010))
            .unwrap()
            .collision
            .grid
            .at(1, 1)
            .unwrap()
            .code(),
        new_code
    );

    // Strict default diff_packs round-trips with a tiny sparse cell patch (NOT 64x64 grid).
    let exported = scratch.join("exported");
    let summary = diff_packs(
        &base,
        &applied.effective_pack,
        &exported,
        &ModManifest::new("exported", "Exported", "1.0.0"),
    )
    .unwrap();
    assert!(summary.derived_items.is_empty());
    let exported_text = fs::read_to_string(exported.join("patches/maps/010_Piata.json")).unwrap();
    assert!(
        exported_text.len() < 256,
        "sparse single-cell patch must stay compact, got {} bytes: {exported_text}",
        exported_text.len()
    );
    let exported_json: Value = serde_json::from_str(&exported_text).unwrap();
    assert_eq!(
        exported_json,
        json!({"collision": {"rows": {"cells": [[1, 1, new_code]]}}})
    );

    let reapplied = apply_mods(
        &base,
        &[load_mod(&exported).unwrap()],
        &scratch.join("reapplied"),
    )
    .unwrap();
    let re_game = GameData::load(&reapplied.effective_pack).unwrap();
    assert_eq!(
        re_game
            .map(crate::MapId(0x010))
            .unwrap()
            .collision
            .grid
            .at(1, 1)
            .unwrap()
            .code(),
        new_code
    );
    if ia_rel != "maps/010_Piata.json" {
        assert_eq!(
            re_game.map(ia_map_id).unwrap().interaction_areas[0].parameter,
            ia.parameter.wrapping_add(1)
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}
