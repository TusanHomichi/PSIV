use super::*;
use serde_json::json;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

fn temp_dir(label: &str) -> PathBuf {
    let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("psiv-mods-test-{label}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn real_pack() -> Option<PathBuf> {
    let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
    pack.join("manifest.json").is_file().then_some(pack)
}

#[test]
fn id_keyed_patch_and_diff_round_trip() {
    let base = json!({
        "format_version": 2,
        "count": 2,
        "enemies": [
            {"id": 10, "symbol": "ZoranBult", "hp": 25, "stats": {"attack": 12, "defense": 4}},
            {"id": 11, "symbol": "MonsterFly", "hp": 18, "stats": {"attack": 9, "defense": 3}}
        ]
    });
    let mut edited = base.clone();
    edited["enemies"][0]["hp"] = json!(99);
    edited["enemies"][0]["stats"]["attack"] = json!(20);

    let patch = diff_json(&base, &edited, "battle/enemies.json")
        .unwrap()
        .expect("non-empty diff");
    assert_eq!(
        patch,
        json!({"enemies": {"10": {"hp": 99, "stats": {"attack": 20}}}})
    );

    let mut applied = base.clone();
    apply_json_patch(&mut applied, &patch, "battle/enemies.json").unwrap();
    assert_eq!(applied, edited);

    // Negative controls: adding or removing records in an ID-keyed table is rejected.
    let mut with_added = base.clone();
    with_added["enemies"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": 99, "symbol": "NewMob", "hp": 50}));
    let add_err = diff_json(&base, &with_added, "battle/enemies.json")
        .expect_err("adding a keyed record must fail diff");
    assert!(add_err.contains("99"), "{add_err}");

    let mut with_removed = base.clone();
    with_removed["enemies"].as_array_mut().unwrap().pop();
    let rm_err = diff_json(&base, &with_removed, "battle/enemies.json")
        .expect_err("removing a keyed record must fail diff");
    assert!(rm_err.contains("11"), "{rm_err}");

    let mut reordered = base.clone();
    reordered["enemies"].as_array_mut().unwrap().reverse();
    let reorder_err = diff_json(&base, &reordered, "battle/enemies.json")
        .expect_err("reordering keyed records must fail diff");
    assert!(reorder_err.contains("reordering"), "{reorder_err}");
}

#[test]
fn dialogue_replace_and_lines_update_segments_pages_and_text() {
    let mut trees = json!({
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
                    "portrait": 2
                }]
            }]
        }]
    });

    let replace_patch = json!({
        "trees": {
            "34": {
                "entries": {
                    "0": {
                        "replace": [["Thank goodness you're safe!", "Glad you're okay!"]]
                    }
                }
            }
        }
    });
    apply_json_patch(&mut trees, &replace_patch, "dialogue/trees.json").unwrap();
    let entry = &trees["trees"][0]["entries"][0];
    assert_eq!(entry["text"], "Professor!\nGlad you're okay!");
    assert_eq!(entry["segments"][3]["text"], "Glad you're okay!");
    assert_eq!(entry["pages"][0]["lines"][1], "Glad you're okay!");

    let lines_patch = json!({
        "trees": {
            "34": {
                "entries": {
                    "0": {
                        "lines": [["Hello!", "Modded world!"], ["Page two!"]]
                    }
                }
            }
        }
    });
    apply_json_patch(&mut trees, &lines_patch, "dialogue/trees.json").unwrap();
    let entry = &trees["trees"][0]["entries"][0];
    assert_eq!(entry["text"], "Hello!\nModded world!\nPage two!");
    assert_eq!(entry["segments"][0]["ctrl"], "portrait");
    assert_eq!(entry["segments"][1]["text"], "Hello!");
    assert_eq!(entry["segments"][2]["ctrl"], "newline");
    assert_eq!(entry["segments"][3]["text"], "Modded world!");
    assert_eq!(entry["segments"][4]["ctrl"], "wait");
    assert_eq!(entry["segments"][5]["text"], "Page two!");
    assert_eq!(
        entry["pages"],
        json!([
            {"lines": ["Hello!", "Modded world!"], "end": "wait"},
            {"lines": ["Page two!"], "end": "end"}
        ])
    );
}

#[test]
fn discover_mods_orders_by_order_then_id_and_skips_disabled() {
    let dir = temp_dir("discover");
    let mut a = ModManifest::new("zeta", "Zeta", "1.0.0");
    a.order = 10;
    let mut b = ModManifest::new("alpha", "Alpha", "1.0.0");
    b.order = 10;
    let mut c = ModManifest::new("off", "Off", "1.0.0");
    c.enabled = false;
    init_mod_dir(&dir.join("zeta"), &a).unwrap();
    init_mod_dir(&dir.join("alpha"), &b).unwrap();
    init_mod_dir(&dir.join("off"), &c).unwrap();
    fs::create_dir_all(dir.join("_active_pack")).unwrap();

    let found = discover_mods(&dir).unwrap();
    let ids: Vec<&str> = found.iter().map(|m| m.manifest.id.as_str()).collect();
    assert_eq!(ids, vec!["alpha", "zeta"]);

    // Duplicate enabled mod ID in another folder must be rejected deterministically.
    init_mod_dir(&dir.join("alpha-copy"), &b).unwrap();
    let dup_err = discover_mods(&dir).expect_err("duplicate enabled mod id must be rejected");
    assert_eq!(dup_err.mod_id, "alpha");
    assert!(dup_err.detail.contains("duplicate mod id"), "{dup_err}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn apply_mods_layers_table_patch_dialogue_and_asset_and_rejects_bad_schema() {
    let Some(base) = real_pack() else {
        eprintln!("runtime-pack absent; skipping real-pack mod overlay test");
        return;
    };
    let scratch = temp_dir("real-overlay");
    let mod_dir = scratch.join("mods/rebalance");
    init_mod_dir(
        &mod_dir,
        &ModManifest::new("rebalance", "Rebalance", "1.0.0"),
    )
    .unwrap();

    // 1. Patch enemy 10 HP to 99.
    fs::create_dir_all(mod_dir.join("patches/battle")).unwrap();
    fs::write(
        mod_dir.join("patches/battle/enemies.json"),
        serde_json::to_string_pretty(&json!({
            "enemies": {"10": {"hp": 99}}
        }))
        .unwrap(),
    )
    .unwrap();

    // 2. Patch dialogue tree 34 entry 0 via replace shorthand, and entry 1 via lines shorthand.
    fs::create_dir_all(mod_dir.join("patches/dialogue")).unwrap();
    fs::write(
        mod_dir.join("patches/dialogue/trees.json"),
        serde_json::to_string_pretty(&json!({
            "trees": {
                "34": {
                    "entries": {
                        "0": {
                            "replace": [["Thank goodness you're safe!", "Glad to see you in one piece!"]]
                        },
                        "1": {
                            "lines": [["Hello!", "Modded lines!"], ["Second page."]]
                        }
                    }
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    // 3. Replace a PNG asset (re-using a valid tiny marker byte sequence).
    let sample_asset_rel = Path::new("maps/010_Piata.png");
    let original_bytes = fs::read(base.join(sample_asset_rel)).unwrap();
    let asset_dst = mod_dir.join("assets").join(sample_asset_rel);
    fs::create_dir_all(asset_dst.parent().unwrap()).unwrap();
    let mut custom_bytes = original_bytes.clone();
    custom_bytes.push(0x42);
    fs::write(&asset_dst, &custom_bytes).unwrap();

    let mods = discover_mods(&scratch.join("mods")).unwrap();
    let overlay_dir = scratch.join("effective");
    let applied = apply_mods(&base, &mods, &overlay_dir).unwrap();
    assert_eq!(applied.active.len(), 1);
    assert_eq!(
        applied.patched_files,
        vec!["battle/enemies.json", "dialogue/trees.json"]
    );
    assert_eq!(applied.replaced_assets, vec!["maps/010_Piata.png"]);

    // Loaded BattleFiles and GameData reflect the edits; base pack is untouched.
    let battle = BattleFiles::load(&applied.effective_pack).unwrap();
    let zoran = battle.enemies.enemies.iter().find(|e| e.id == 10).unwrap();
    assert_eq!(zoran.hp, 99);
    let base_battle = BattleFiles::load(&base).unwrap();
    let base_zoran = base_battle
        .enemies
        .enemies
        .iter()
        .find(|e| e.id == 10)
        .unwrap();
    assert_eq!(base_zoran.hp, 25, "base pack must never be mutated");

    let game = GameData::load(&applied.effective_pack).unwrap();
    let dialogue = game.dialogue().unwrap();
    let line = &dialogue
        .trees
        .trees
        .iter()
        .find(|t| t.tree == 34)
        .unwrap()
        .entry(0)
        .unwrap()
        .text;
    assert!(line.contains("Glad to see you in one piece!"), "{line}");
    assert_eq!(
        fs::read(applied.effective_pack.join(sample_asset_rel)).unwrap(),
        custom_bytes
    );

    // Negative control 1: schema-breaking HP > u16::MAX names mod, file, and place.
    let bad_mod = scratch.join("bad-hp");
    init_mod_dir(&bad_mod, &ModManifest::new("bad-hp", "Bad HP", "1.0.0")).unwrap();
    fs::create_dir_all(bad_mod.join("patches/battle")).unwrap();
    fs::write(
        bad_mod.join("patches/battle/enemies.json"),
        r#"{"enemies":{"10":{"hp":70000}}}"#,
    )
    .unwrap();
    let err = apply_mods(
        &base,
        &[load_mod(&bad_mod).unwrap()],
        &scratch.join("bad-out"),
    )
    .expect_err("HP 70000 must be refused");
    assert_eq!(err.mod_id, "bad-hp");
    assert_eq!(err.file, "battle/enemies.json");
    assert!(err.detail.contains("70000"), "{err}");

    // Negative control 2: unknown enemy id is refused with mod and file named.
    let bad_id = scratch.join("bad-id");
    init_mod_dir(&bad_id, &ModManifest::new("bad-id", "Bad ID", "1.0.0")).unwrap();
    fs::create_dir_all(bad_id.join("patches/battle")).unwrap();
    fs::write(
        bad_id.join("patches/battle/enemies.json"),
        r#"{"enemies":{"9999":{"hp":10}}}"#,
    )
    .unwrap();
    let err = apply_mods(
        &base,
        &[load_mod(&bad_id).unwrap()],
        &scratch.join("bad-id-out"),
    )
    .expect_err("unknown enemy id must be refused");
    assert_eq!(err.mod_id, "bad-id");
    assert!(err.file.contains("battle/enemies.json"), "{err}");
    assert!(err.detail.contains("9999"), "{err}");

    // Round-trip: diffing the effective pack back against base reproduces the edits.
    let exported = scratch.join("exported-mod");
    let summary = diff_packs(
        &base,
        &applied.effective_pack,
        &exported,
        &ModManifest::new("exported", "Exported", "1.0.0"),
    )
    .unwrap();
    assert_eq!(
        summary.patches,
        vec!["battle/enemies.json", "dialogue/trees.json"]
    );
    assert_eq!(summary.assets, vec!["maps/010_Piata.png"]);
    let reapplied = apply_mods(
        &base,
        &[load_mod(&exported).unwrap()],
        &scratch.join("reapplied"),
    )
    .unwrap();
    let re_battle = BattleFiles::load(&reapplied.effective_pack).unwrap();
    assert_eq!(
        re_battle
            .enemies
            .enemies
            .iter()
            .find(|e| e.id == 10)
            .unwrap()
            .hp,
        99
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn mod_save_profile_slug_is_stable_across_versions_and_isolates_combos() {
    assert_eq!(mod_save_profile_slug(&[]), None);

    let v1 = ModManifest::new("hard-mode", "Hard Mode", "1.0.0");
    let v2 = ModManifest::new("hard-mode", "Hard Mode", "2.0.0");
    assert_eq!(
        mod_save_profile_slug(std::slice::from_ref(&v1)).as_deref(),
        Some("hard-mode")
    );
    assert_eq!(
        mod_save_profile_slug(std::slice::from_ref(&v1)),
        mod_save_profile_slug(&[v2]),
        "updating a mod version must keep the same save profile directory"
    );

    let second = ModManifest::new("new-music", "New Music", "1.0.0");
    assert_eq!(
        mod_save_profile_slug(&[v1.clone(), second.clone()]).as_deref(),
        Some("hard-mode+new-music")
    );
    assert_ne!(
        mod_save_profile_slug(&[v1.clone(), second.clone()]),
        mod_save_profile_slug(&[second, v1]),
        "load order matters for mod overlays"
    );

    let long_a = ModManifest::new("very-long-rebalance-overhaul-mod", "A", "1.0.0");
    let long_b = ModManifest::new("another-lengthy-dialogue-rewrite", "B", "1.0.0");
    let hashed = mod_save_profile_slug(&[long_a, long_b]).unwrap();
    assert!(hashed.starts_with("very-long-rebala~2mods~"), "{hashed}");
    assert!(
        !hashed.contains('+'),
        "hashed slug must be disjoint from joined IDs: {hashed}"
    );
    assert!(hashed.len() <= 48, "{hashed}");

    // Windows reserved device names are rejected as mod IDs and defensively
    // encoded if passed directly to mod_save_profile_slug.
    for reserved in ["con", "prn", "aux", "nul", "com1", "lpt9"] {
        assert!(!is_valid_mod_id(reserved), "{reserved} must be rejected");
        let slug = mod_save_profile_slug(&[ModManifest::new(reserved, "R", "1.0.0")]).unwrap();
        assert_eq!(slug, format!("~{reserved}"));
    }
    assert!(is_valid_mod_id("console"));
    assert!(is_valid_mod_id("com10"));
}

#[test]
fn mod_profile_sidecar_round_trips_and_reports_version_changes() {
    let dir = temp_dir("profile-sidecar");
    assert_eq!(ModProfile::read_from_dir(&dir).unwrap(), None);

    let p1 = ModProfile::from_manifests(&[
        ModManifest::new("hard-mode", "Hard Mode", "1.0.0"),
        ModManifest::new("alt-font", "Alt Font", "0.5.0"),
    ]);
    p1.write_to_dir(&dir).unwrap();
    assert_eq!(ModProfile::read_from_dir(&dir).unwrap().as_ref(), Some(&p1));

    let p2 = ModProfile::from_manifests(&[
        ModManifest::new("hard-mode", "Hard Mode", "1.1.0"),
        ModManifest::new("alt-font", "Alt Font", "0.5.0"),
    ]);
    let diff = p2.diff_against(&p1);
    assert!(!diff.is_empty());
    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());
    assert!(!diff.order_changed);
    assert_eq!(
        diff.version_changes,
        vec![("hard-mode".into(), "1.0.0".into(), "1.1.0".into())]
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn diff_packs_clears_stale_patches_and_assets_on_reexport() {
    let scratch = temp_dir("diff-reexport");
    let base = scratch.join("base");
    let edited = scratch.join("edited");
    let out_mod = scratch.join("mod");

    fs::create_dir_all(base.join("battle")).unwrap();
    fs::create_dir_all(base.join("maps")).unwrap();
    fs::write(base.join("manifest.json"), "{}\n").unwrap();
    fs::write(
        base.join("battle/enemies.json"),
        r#"{"enemies":[{"id":10,"hp":25}]}"#,
    )
    .unwrap();
    fs::write(base.join("maps/test.png"), b"base-png").unwrap();

    fs::create_dir_all(edited.join("battle")).unwrap();
    fs::create_dir_all(edited.join("maps")).unwrap();
    fs::write(edited.join("manifest.json"), "{}\n").unwrap();
    fs::write(
        edited.join("battle/enemies.json"),
        r#"{"enemies":[{"id":10,"hp":99}]}"#,
    )
    .unwrap();
    fs::write(edited.join("maps/test.png"), b"mod-png").unwrap();

    let manifest = ModManifest::new("reexport", "Reexport", "1.0.0");
    let first = diff_packs(&base, &edited, &out_mod, &manifest).unwrap();
    assert_eq!(first.patches, vec!["battle/enemies.json"]);
    assert_eq!(first.assets, vec!["maps/test.png"]);
    assert!(out_mod.join("patches/battle/enemies.json").is_file());
    assert!(out_mod.join("assets/maps/test.png").is_file());

    // Revert both edits and differ manifest.json; manifest.json must be skipped
    // rather than emitted into patches/manifest.json.
    fs::write(
        edited.join("manifest.json"),
        "{\"format_version\": 2, \"note\": \"rebuilt\"}\n",
    )
    .unwrap();
    fs::write(
        edited.join("battle/enemies.json"),
        r#"{"enemies":[{"id":10,"hp":25}]}"#,
    )
    .unwrap();
    fs::write(edited.join("maps/test.png"), b"base-png").unwrap();

    let second = diff_packs(&base, &edited, &out_mod, &manifest).unwrap();
    assert!(second.patches.is_empty());
    assert!(second.assets.is_empty());
    assert!(
        !out_mod.join("patches/manifest.json").exists(),
        "pack manifest.json must never be emitted as a mod patch"
    );
    assert!(
        !out_mod.join("patches/battle/enemies.json").exists(),
        "stale patch file must be removed on re-export"
    );
    assert!(
        !out_mod.join("assets/maps/test.png").exists(),
        "stale asset file must be removed on re-export"
    );

    // Deleting a base pack file from edited_pack must fail closed.
    fs::remove_file(edited.join("maps/test.png")).unwrap();
    let del_err = diff_packs(&base, &edited, &out_mod, &manifest)
        .expect_err("deleting a base pack file must be rejected");
    assert_eq!(del_err.file, "maps/test.png");
    assert!(del_err.detail.contains("deleting"), "{del_err}");

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn overlapping_output_paths_are_rejected_without_touching_sources() {
    let scratch = temp_dir("overlap-guard");
    let base = scratch.join("base-pack");
    let mod_dir = scratch.join("mods/alpha");
    fs::create_dir_all(&base).unwrap();
    fs::write(base.join("manifest.json"), "{}\n").unwrap();
    fs::write(base.join("sentinel.txt"), "keep-me").unwrap();

    let manifest = ModManifest::new("alpha", "Alpha", "1.0.0");
    init_mod_dir(&mod_dir, &manifest).unwrap();
    let loaded = [load_mod(&mod_dir).unwrap()];

    // Equal, descendant, ancestor, and mod-root overlaps must all be refused.
    for bad_out in [
        base.clone(),
        base.join("nested-overlay"),
        scratch.clone(),
        mod_dir.clone(),
        mod_dir.join("sub"),
    ] {
        let err = apply_mods(&base, &loaded, &bad_out)
            .expect_err("overlapping output dir must be rejected");
        assert!(err.detail.contains("overlaps"), "{err}");
    }
    assert_eq!(
        fs::read_to_string(base.join("sentinel.txt")).unwrap(),
        "keep-me",
        "base pack must remain completely untouched"
    );

    // diff_packs must also reject overlapping base/edited pack outputs.
    let edited = scratch.join("edited-pack");
    fs::create_dir_all(&edited).unwrap();
    fs::write(edited.join("manifest.json"), "{}\n").unwrap();
    for bad_out in [
        base.clone(),
        base.join("out"),
        edited.clone(),
        scratch.clone(),
    ] {
        let err = diff_packs(&base, &edited, &bad_out, &manifest)
            .expect_err("diff_packs must reject overlapping output");
        assert!(err.detail.contains("overlaps"), "{err}");
    }

    let _ = fs::remove_dir_all(&scratch);
}

#[cfg(unix)]
#[test]
fn symlinks_inside_mod_folders_are_rejected_without_recursing() {
    let scratch = temp_dir("mod-symlink-guard");
    let base = scratch.join("base-pack");
    let mod_dir = scratch.join("mods/loop-mod");
    fs::create_dir_all(&base).unwrap();
    fs::write(base.join("manifest.json"), "{}\n").unwrap();

    let manifest = ModManifest::new("loop-mod", "Loop Mod", "1.0.0");
    init_mod_dir(&mod_dir, &manifest).unwrap();
    std::os::unix::fs::symlink(&mod_dir, mod_dir.join("patches/self-loop")).unwrap();

    let loaded = [load_mod(&mod_dir).unwrap()];
    let err = apply_mods(&base, &loaded, &scratch.join("out"))
        .expect_err("cyclic symlink in mod patches must be rejected");
    assert_eq!(err.mod_id, "loop-mod");
    assert!(err.detail.contains("symlinks"), "{err}");

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn reconcile_map_update_images_respects_png_and_indices_precedence() {
    let scratch = temp_dir("reconcile-map-art");
    let base_map = json!({
        "format_version": 2,
        "id": 16,
        "png": "maps/010_Piata.png",
        "map_update_images": {
            "maps/010_Piata.png": "maps/010_Piata_indices.png",
            "maps/010_Piata_over.png": "maps/010_Piata_over_indices.png"
        }
    });

    // Helper: write a fresh overlay maps/010_Piata.json symlinked to a read-only
    // base copy, run reconciliation with `layers`, and return the resulting map JSON.
    let run_case = |label: &str, layers: &[(&str, usize)]| -> serde_json::Value {
        let case_dir = scratch.join(label);
        let base_dir = case_dir.join("base/maps");
        let out_dir = case_dir.join("out/maps");
        fs::create_dir_all(&base_dir).unwrap();
        fs::create_dir_all(&out_dir).unwrap();
        let base_file = base_dir.join("010_Piata.json");
        fs::write(&base_file, serde_json::to_string(&base_map).unwrap()).unwrap();
        let out_file = out_dir.join("010_Piata.json");
        link_or_copy(&base_file, &out_file).unwrap();

        let asset_layer: BTreeMap<String, usize> =
            layers.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
        map_art::reconcile_replaced_map_update_images(&case_dir.join("out"), &asset_layer).unwrap();

        // Base map JSON must never be mutated through the link.
        let base_after: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&base_file).unwrap()).unwrap();
        assert_eq!(base_after, base_map, "base map JSON must stay untouched");

        serde_json::from_str(&fs::read_to_string(&out_file).unwrap()).unwrap()
    };

    // 1. Replacing `.png` alone drops its redirect so the renderer uses the mod PNG,
    //    while keeping the untouched overlay redirect intact.
    let png_only = run_case("png-only", &[("maps/010_Piata.png", 0)]);
    let mui = png_only["map_update_images"].as_object().unwrap();
    assert!(!mui.contains_key("maps/010_Piata.png"));
    assert_eq!(
        mui.get("maps/010_Piata_over.png").and_then(|v| v.as_str()),
        Some("maps/010_Piata_over_indices.png")
    );

    // 2. Replacing `_indices.png` explicitly (alone or in the same mod as `.png`)
    //    keeps the redirect so the renderer loads the mod's `_indices.png`.
    let idx_only = run_case("idx-only", &[("maps/010_Piata_indices.png", 0)]);
    assert_eq!(
        idx_only["map_update_images"]["maps/010_Piata.png"],
        "maps/010_Piata_indices.png"
    );
    let same_mod_both = run_case(
        "both-same-mod",
        &[("maps/010_Piata.png", 0), ("maps/010_Piata_indices.png", 0)],
    );
    assert_eq!(
        same_mod_both["map_update_images"]["maps/010_Piata.png"],
        "maps/010_Piata_indices.png"
    );

    // 3. Multi-mod precedence: later `.png` beats earlier `_indices.png`;
    //    later `_indices.png` beats earlier `.png`.
    let png_wins = run_case(
        "png-over-idx",
        &[("maps/010_Piata_indices.png", 0), ("maps/010_Piata.png", 1)],
    );
    assert!(
        !png_wins["map_update_images"]
            .as_object()
            .unwrap()
            .contains_key("maps/010_Piata.png")
    );
    let idx_wins = run_case(
        "idx-over-png",
        &[("maps/010_Piata.png", 0), ("maps/010_Piata_indices.png", 1)],
    );
    assert_eq!(
        idx_wins["map_update_images"]["maps/010_Piata.png"],
        "maps/010_Piata_indices.png"
    );

    // 4. `diff_packs` omits synthetic `map_update_images` removals when round-tripping.
    let mut patch_val = diff_json(&base_map, &png_only, "maps/010_Piata.json")
        .unwrap()
        .expect("reconciled JSON differs from base");
    let changed_assets = std::collections::BTreeSet::from(["maps/010_Piata.png".to_owned()]);
    assert!(
        !map_art::strip_reconciled_map_update_removals(
            "maps/010_Piata.json",
            &base_map,
            &mut patch_val,
            &changed_assets,
        ),
        "synthetic map_update_images removal should not emit a spurious map patch"
    );
    // Negative control: deleting a redirect WITHOUT replacing the PNG is preserved.
    let mut manual_patch = diff_json(&base_map, &png_only, "maps/010_Piata.json")
        .unwrap()
        .unwrap();
    let no_assets = std::collections::BTreeSet::new();
    assert!(map_art::strip_reconciled_map_update_removals(
        "maps/010_Piata.json",
        &base_map,
        &mut manual_patch,
        &no_assets,
    ));
    assert!(
        manual_patch["map_update_images"]["maps/010_Piata.png"].is_null(),
        "explicit redirect deletion without asset replacement must be preserved"
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn apply_mods_reconciles_live_palette_map_png_replacement_on_real_pack() {
    let Some(base) = real_pack() else {
        eprintln!("runtime-pack absent; skipping real-pack live-palette map PNG test");
        return;
    };

    // Proof that the base pack actually carries the redirect we are reconciling.
    let base_data = GameData::load(&base).unwrap();
    let base_piata = base_data.map(crate::MapId(0x010)).unwrap();
    assert_eq!(
        base_piata
            .map_update_images
            .get("maps/010_Piata.png")
            .map(String::as_str),
        Some("maps/010_Piata_indices.png")
    );

    let scratch = temp_dir("real-map-png");
    let png_mod = scratch.join("mods/piata-png");
    init_mod_dir(
        &png_mod,
        &ModManifest::new("piata-png", "Piata PNG", "1.0.0"),
    )
    .unwrap();
    let mut custom_png = fs::read(base.join("maps/010_Piata.png")).unwrap();
    custom_png.push(0x99);
    fs::create_dir_all(png_mod.join("assets/maps")).unwrap();
    fs::write(png_mod.join("assets/maps/010_Piata.png"), &custom_png).unwrap();

    let applied_png = apply_mods(
        &base,
        &[load_mod(&png_mod).unwrap()],
        &scratch.join("effective-png"),
    )
    .unwrap();
    let effective_data = GameData::load(&applied_png.effective_pack).unwrap();
    let piata = effective_data.map(crate::MapId(0x010)).unwrap();
    assert!(
        !piata.map_update_images.contains_key("maps/010_Piata.png"),
        "replacing 010_Piata.png must drop the stale _indices redirect"
    );
    assert_eq!(
        piata
            .map_update_images
            .get("maps/010_Piata_over.png")
            .map(String::as_str),
        Some("maps/010_Piata_over_indices.png"),
        "untouched overlay redirect must stay intact"
    );

    // Explicitly replacing `_indices.png` keeps the redirect pointing at the mod's file.
    let idx_mod = scratch.join("mods/piata-idx");
    init_mod_dir(
        &idx_mod,
        &ModManifest::new("piata-idx", "Piata Indices", "1.0.0"),
    )
    .unwrap();
    let mut custom_idx = fs::read(base.join("maps/010_Piata_indices.png")).unwrap();
    custom_idx.push(0x77);
    fs::create_dir_all(idx_mod.join("assets/maps")).unwrap();
    fs::write(
        idx_mod.join("assets/maps/010_Piata_indices.png"),
        &custom_idx,
    )
    .unwrap();

    let applied_idx = apply_mods(
        &base,
        &[load_mod(&idx_mod).unwrap()],
        &scratch.join("effective-idx"),
    )
    .unwrap();
    let effective_idx = GameData::load(&applied_idx.effective_pack).unwrap();
    let piata_idx = effective_idx.map(crate::MapId(0x010)).unwrap();
    assert_eq!(
        piata_idx
            .map_update_images
            .get("maps/010_Piata.png")
            .map(String::as_str),
        Some("maps/010_Piata_indices.png")
    );
    assert_eq!(
        fs::read(
            applied_idx
                .effective_pack
                .join("maps/010_Piata_indices.png")
        )
        .unwrap(),
        custom_idx
    );

    // Base pack remains completely untouched.
    let base_recheck = GameData::load(&base).unwrap();
    assert_eq!(
        base_recheck
            .map(crate::MapId(0x010))
            .unwrap()
            .map_update_images
            .get("maps/010_Piata.png")
            .map(String::as_str),
        Some("maps/010_Piata_indices.png")
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn inspect_mods_dir_lists_enabled_disabled_and_invalid_and_set_enabled_persists() {
    let dir = temp_dir("inventory");
    assert!(inspect_mods_dir(&dir.join("missing")).unwrap().is_empty());

    let mut alpha = ModManifest::new("alpha", "Alpha", "1.0.0");
    alpha.order = 10;
    let mut beta = ModManifest::new("beta", "Beta", "2.0.0");
    beta.order = 20;
    beta.enabled = false;
    init_mod_dir(&dir.join("alpha"), &alpha).unwrap();
    init_mod_dir(&dir.join("beta"), &beta).unwrap();

    // Add an invalid mod with broken JSON and an outdated pack_format_version mod.
    fs::create_dir_all(dir.join("broken")).unwrap();
    fs::write(dir.join("broken/mod.json"), "{not-json").unwrap();

    let mut outdated = ModManifest::new("old-mod", "Old Mod", "0.1.0");
    outdated.pack_format_version = 999;
    fs::create_dir_all(dir.join("old-mod")).unwrap();
    fs::write(
        dir.join("old-mod/mod.json"),
        serde_json::to_string_pretty(&outdated).unwrap(),
    )
    .unwrap();

    let entries = inspect_mods_dir(&dir).unwrap();
    assert_eq!(entries.len(), 4);
    assert!(
        matches!(&entries[0], ModInventoryEntry::Valid(m) if m.manifest.id == "alpha" && m.manifest.enabled)
    );
    assert!(
        matches!(&entries[1], ModInventoryEntry::Valid(m) if m.manifest.id == "beta" && !m.manifest.enabled)
    );
    assert!(
        matches!(&entries[2], ModInventoryEntry::Invalid { root, .. } if root == &dir.join("broken"))
    );
    assert!(
        matches!(&entries[3], ModInventoryEntry::Invalid { root, .. } if root == &dir.join("old-mod"))
    );

    // Disabling an outdated mod is allowed and unblocks discover_mods; re-enabling it is refused.
    assert_eq!(
        find_mod_dir_by_id(&dir, "old-mod"),
        Some(dir.join("old-mod"))
    );
    let disabled_old = set_mod_enabled(&dir.join("old-mod"), false).unwrap();
    assert!(!disabled_old.enabled);
    assert_eq!(find_mod_dir_by_id(&dir, "old-mod"), None);
    let reenable_err = set_mod_enabled(&dir.join("old-mod"), true)
        .expect_err("enabling an incompatible pack_format_version mod must be refused");
    assert_eq!(reenable_err.mod_id, "old-mod");
    assert!(
        reenable_err.detail.contains("pack format_version"),
        "{reenable_err}"
    );

    // Toggle beta on and alpha off; verify persistence through load_mod.
    let beta_on = set_mod_enabled(&dir.join("beta"), true).unwrap();
    assert!(beta_on.enabled);
    let alpha_off = set_mod_enabled(&dir.join("alpha"), false).unwrap();
    assert!(!alpha_off.enabled);
    assert!(load_mod(&dir.join("beta")).unwrap().manifest.enabled);
    assert!(!load_mod(&dir.join("alpha")).unwrap().manifest.enabled);

    // Negative control: unparseable JSON cannot be toggled or located as disableable.
    assert!(set_mod_enabled(&dir.join("broken"), false).is_err());
    assert_eq!(
        find_mod_dir_by_id(&dir, &dir.join("broken").display().to_string()),
        None
    );

    // Negative control: duplicate enabled IDs return None so recovery does not guess.
    fs::remove_dir_all(dir.join("broken")).unwrap();
    init_mod_dir(&dir.join("beta-dup"), &beta_on).unwrap();
    assert_eq!(find_mod_dir_by_id(&dir, "beta"), None);

    // Toggling preserves any extra/unmodeled keys in mod.json instead of dropping them.
    let extra_dir = dir.join("extra-meta");
    fs::create_dir_all(&extra_dir).unwrap();
    fs::write(
        extra_dir.join(MOD_MANIFEST_FILE),
        r#"{"format_version":1,"id":"extra-meta","name":"Extra","version":"1.0.0","enabled":true,"homepage":"https://example.invalid/mod","custom_nested":{"tag":"v1"}}"#,
    )
    .unwrap();
    let toggled = set_mod_enabled(&extra_dir, false).unwrap();
    assert!(!toggled.enabled);
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(extra_dir.join(MOD_MANIFEST_FILE)).unwrap())
            .unwrap();
    assert_eq!(saved["enabled"], serde_json::Value::Bool(false));
    assert_eq!(saved["homepage"], "https://example.invalid/mod");
    assert_eq!(saved["custom_nested"]["tag"], "v1");

    let _ = fs::remove_dir_all(&dir);
}
