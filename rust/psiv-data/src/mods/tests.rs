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

    let patch = diff_json(&base, &edited, "battle/enemies.json").expect("non-empty diff");
    assert_eq!(
        patch,
        json!({"enemies": {"10": {"hp": 99, "stats": {"attack": 20}}}})
    );

    let mut applied = base.clone();
    apply_json_patch(&mut applied, &patch, "battle/enemies.json").unwrap();
    assert_eq!(applied, edited);
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
                        "lines": ["Hello!", "Modded world!"]
                    }
                }
            }
        }
    });
    apply_json_patch(&mut trees, &lines_patch, "dialogue/trees.json").unwrap();
    let entry = &trees["trees"][0]["entries"][0];
    assert_eq!(entry["text"], "Hello!\nModded world!");
    assert_eq!(entry["segments"][0]["ctrl"], "portrait");
    assert_eq!(entry["segments"][1]["text"], "Hello!");
    assert_eq!(entry["segments"][2]["ctrl"], "newline");
    assert_eq!(entry["segments"][3]["text"], "Modded world!");
    assert_eq!(
        entry["pages"][0]["lines"],
        json!(["Hello!", "Modded world!"])
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

    // 2. Patch dialogue tree 34 entry 0 via replace shorthand.
    fs::create_dir_all(mod_dir.join("patches/dialogue")).unwrap();
    fs::write(
        mod_dir.join("patches/dialogue/trees.json"),
        serde_json::to_string_pretty(&json!({
            "trees": {
                "34": {
                    "entries": {
                        "0": {
                            "replace": [["Thank goodness you're safe!", "Glad to see you in one piece!"]]
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
    assert!(hashed.starts_with("very-long-rebala+2mods+"), "{hashed}");
    assert!(hashed.len() <= 48, "{hashed}");
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
