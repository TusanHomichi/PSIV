//! CLI tests for `psiv-pack mod <init|diff|check|schema>` (#118, #790).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

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

#[test]
fn mod_init_diff_and_check_round_trip_and_refuse_invalid_mod() {
    let Some(pack) = real_pack() else {
        eprintln!("runtime-pack absent; skipping psiv-pack mod CLI test");
        return;
    };

    let binary = env!("CARGO_BIN_EXE_psiv-pack");
    let scratch = std::env::temp_dir().join(format!("psiv-pack-mod-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).unwrap();

    // 1. `psiv-pack mod init`
    let init_dir = scratch.join("starter");
    let out = Command::new(binary)
        .args(["mod", "init"])
        .arg(&init_dir)
        .args(["--id", "starter", "--name", "Starter Mod"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "mod init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(init_dir.join("mod.json").is_file());

    // 2. Patch enemy 10 HP and a real map's treasure_chests by index, then `psiv-pack mod check`
    let base_game = psiv_data::GameData::load(&pack).unwrap();
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
    let chest = &chest_map.treasure_chests[0];

    fs::create_dir_all(init_dir.join("patches/battle")).unwrap();
    fs::write(
        init_dir.join("patches/battle/enemies.json"),
        r#"{"enemies":{"10":{"hp":77}}}"#,
    )
    .unwrap();
    let chest_patch_file = init_dir.join("patches").join(&chest_map_rel);
    fs::create_dir_all(chest_patch_file.parent().unwrap()).unwrap();
    fs::write(
        &chest_patch_file,
        format!(
            r#"{{"treasure_chests":{{"{}":{{"white_chest":{}}}}}}}"#,
            chest.index, !chest.white_chest
        ),
    )
    .unwrap();
    let check = Command::new(binary)
        .args(["mod", "check"])
        .arg(&pack)
        .arg(&init_dir)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "mod check failed: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    let stdout = String::from_utf8_lossy(&check.stdout);
    assert!(
        stdout.contains("ok: 1 mod(s), 2 patched file(s)"),
        "{stdout}"
    );

    // 3. Apply mod to an edited pack, then `psiv-pack mod diff` back to a new mod
    let edited_pack = scratch.join("edited-pack");
    let loaded = psiv_data::mods::load_mod(&init_dir).unwrap();
    psiv_data::mods::apply_mods(&pack, &[loaded], &edited_pack).unwrap();

    let diff_dir = scratch.join("diffed");
    let diff = Command::new(binary)
        .args(["mod", "diff"])
        .arg(&pack)
        .arg(&edited_pack)
        .arg(&diff_dir)
        .args(["--id", "diffed", "--name", "Diffed Mod"])
        .output()
        .unwrap();
    assert!(
        diff.status.success(),
        "mod diff failed: {}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let diffed_chest_patch = fs::read_to_string(diff_dir.join("patches").join(&chest_map_rel))
        .expect("diffed mod must emit map treasure_chests patch");
    assert!(
        diffed_chest_patch.contains("\"treasure_chests\""),
        "{diffed_chest_patch}"
    );

    let recheck = Command::new(binary)
        .args(["mod", "check"])
        .arg(&pack)
        .arg(&diff_dir)
        .output()
        .unwrap();
    assert!(
        recheck.status.success(),
        "diffed mod check failed: {}",
        String::from_utf8_lossy(&recheck.stderr)
    );

    // 4. Negative control: broken schema fails with exit code 2 and names mod + file
    fs::write(
        init_dir.join("patches/battle/enemies.json"),
        r#"{"enemies":{"10":{"hp":70000}}}"#,
    )
    .unwrap();
    let bad = Command::new(binary)
        .args(["mod", "check"])
        .arg(&pack)
        .arg(&init_dir)
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(stderr.contains("starter"), "{stderr}");
    assert!(stderr.contains("battle/enemies.json"), "{stderr}");
    assert!(stderr.contains("70000"), "{stderr}");

    // 5. Negative control: typo patch field ("HP": 99) fails with exit code 2 naming mod, file, and field
    fs::write(
        init_dir.join("patches/battle/enemies.json"),
        r#"{"enemies":{"10":{"HP":99}}}"#,
    )
    .unwrap();
    let bad_typo = Command::new(binary)
        .args(["mod", "check"])
        .arg(&pack)
        .arg(&init_dir)
        .output()
        .unwrap();
    assert_eq!(bad_typo.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&bad_typo.stderr);
    assert!(stderr.contains("starter"), "{stderr}");
    assert!(stderr.contains("battle/enemies.json"), "{stderr}");
    assert!(
        stderr.contains("enemies[10].HP") || stderr.contains("enemies.10.HP"),
        "{stderr}"
    );

    // 6. Negative control: unknown field in mod.json fails with exit code 2 naming mod.json and field
    let bad_manifest_dir = scratch.join("bad-manifest");
    fs::create_dir_all(&bad_manifest_dir).unwrap();
    fs::write(
        bad_manifest_dir.join("mod.json"),
        r#"{"format_version":1,"id":"bad-manifest","name":"Bad","version":"1.0.0","typo_flag":true}"#,
    )
    .unwrap();
    let bad_manifest = Command::new(binary)
        .args(["mod", "check"])
        .arg(&pack)
        .arg(&bad_manifest_dir)
        .output()
        .unwrap();
    assert_eq!(bad_manifest.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&bad_manifest.stderr);
    assert!(stderr.contains("mod.json"), "{stderr}");
    assert!(stderr.contains("typo_flag"), "{stderr}");

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn modding_guide_examples_pass_psiv_pack_mod_check() {
    let Some(pack) = real_pack() else {
        eprintln!("runtime-pack absent; skipping MODDING.md CLI check");
        return;
    };

    let guide =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/MODDING.md"))
            .expect("read docs/MODDING.md");

    // Extract every ```json block from docs/MODDING.md in order.
    let mut json_blocks = Vec::new();
    let mut in_json = false;
    let mut current = String::new();
    for line in guide.lines() {
        let trimmed = line.trim();
        if !in_json {
            if trimmed == "```json" {
                in_json = true;
                current.clear();
            }
        } else if trimmed.starts_with("```") {
            json_blocks.push(current.clone());
            in_json = false;
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    assert_eq!(
        json_blocks.len(),
        4,
        "expected manifest + enemy + dialogue + map collision JSON examples in docs/MODDING.md"
    );

    let binary = env!("CARGO_BIN_EXE_psiv-pack");
    let scratch = std::env::temp_dir().join(format!("psiv-pack-mod-guide-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let mod_dir = scratch.join("hard-mode");
    fs::create_dir_all(mod_dir.join("patches/battle")).unwrap();
    fs::create_dir_all(mod_dir.join("patches/dialogue")).unwrap();
    fs::create_dir_all(mod_dir.join("patches/maps")).unwrap();
    fs::write(mod_dir.join("mod.json"), &json_blocks[0]).unwrap();
    fs::write(mod_dir.join("patches/battle/enemies.json"), &json_blocks[1]).unwrap();
    fs::write(mod_dir.join("patches/dialogue/trees.json"), &json_blocks[2]).unwrap();
    fs::write(mod_dir.join("patches/maps/010_Piata.json"), &json_blocks[3]).unwrap();

    let ok = Command::new(binary)
        .args(["mod", "check"])
        .arg(&pack)
        .arg(&mod_dir)
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "psiv-pack mod check failed on docs/MODDING.md examples: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(
        stdout.contains("ok: 1 mod(s), 3 patched file(s)"),
        "{stdout}"
    );

    // Negative control: documented `"hp": 70000` must fail with exit code 2.
    fs::write(
        mod_dir.join("patches/battle/enemies.json"),
        r#"{"enemies":{"10":{"hp":70000}}}"#,
    )
    .unwrap();
    let bad = Command::new(binary)
        .args(["mod", "check"])
        .arg(&pack)
        .arg(&mod_dir)
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(stderr.contains("hard-mode"), "{stderr}");
    assert!(stderr.contains("battle/enemies.json"), "{stderr}");
    assert!(stderr.contains("70000"), "{stderr}");
    assert!(stderr.contains("u16"), "{stderr}");

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn mod_schema_generates_and_checks_sync_without_rom() {
    let binary = env!("CARGO_BIN_EXE_psiv-pack");
    let published = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas");

    // 1. `--check` passes against the committed `docs/schemas/` directory.
    let check_docs = Command::new(binary)
        .args(["mod", "schema"])
        .arg(&published)
        .arg("--check")
        .output()
        .unwrap();
    assert!(
        check_docs.status.success(),
        "psiv-pack mod schema docs/schemas --check failed: {}",
        String::from_utf8_lossy(&check_docs.stderr)
    );

    // 2. Generating into a fresh directory is deterministic and passes `--check`.
    let scratch = std::env::temp_dir().join(format!("psiv-pack-mod-schema-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let generated = Command::new(binary)
        .args(["mod", "schema"])
        .arg(&scratch)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "psiv-pack mod schema failed: {}",
        String::from_utf8_lossy(&generated.stderr)
    );
    assert!(scratch.join("mod.schema.json").is_file());
    assert!(scratch.join("patches/battle/enemies.schema.json").is_file());
    assert!(scratch.join("patches/dialogue/trees.schema.json").is_file());

    let check_fresh = Command::new(binary)
        .args(["mod", "schema"])
        .arg(&scratch)
        .arg("--check")
        .output()
        .unwrap();
    assert!(
        check_fresh.status.success(),
        "fresh schema --check failed: {}",
        String::from_utf8_lossy(&check_fresh.stderr)
    );

    // 3. Negative control: drifting a schema causes `--check` to exit 2.
    fs::write(scratch.join("mod.schema.json"), "{}\n").unwrap();
    let drifted = Command::new(binary)
        .args(["mod", "schema"])
        .arg(&scratch)
        .arg("--check")
        .output()
        .unwrap();
    assert_eq!(drifted.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&drifted.stderr);
    assert!(stderr.contains("mod.schema.json"), "{stderr}");

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn mod_diff_refuses_derived_copies_by_default_and_supports_sparse_cells_and_opt_in() {
    let binary = env!("CARGO_BIN_EXE_psiv-pack");
    let scratch =
        std::env::temp_dir().join(format!("psiv-pack-mod-shareability-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let base = scratch.join("base");
    let edited = scratch.join("edited");
    let out_mod = scratch.join("exported");

    fs::create_dir_all(base.join("maps")).unwrap();
    fs::create_dir_all(edited.join("maps")).unwrap();
    fs::write(base.join("manifest.json"), "{}\n").unwrap();
    fs::write(edited.join("manifest.json"), "{}\n").unwrap();

    let base_map = r#"{
        "format_version": 2,
        "id": 16,
        "collision": {"plane": "bg", "plane_byte": 1, "width_cells": 2, "height_cells": 2, "rows": [[0, 0], [0, 8]]},
        "events": [0]
    }"#;
    fs::write(base.join("maps/010_Piata.json"), base_map).unwrap();
    fs::write(base.join("maps/010_Piata.png"), b"retail-png").unwrap();
    fs::write(edited.join("maps/010_Piata.png"), b"retail-png").unwrap();

    // 1. Single collision cell edit + brand-new asset succeeds in default strict mode.
    let edited_cell = r#"{
        "format_version": 2,
        "id": 16,
        "collision": {"plane": "bg", "plane_byte": 1, "width_cells": 2, "height_cells": 2, "rows": [[0, 8], [0, 8]]},
        "events": [0]
    }"#;
    fs::write(edited.join("maps/010_Piata.json"), edited_cell).unwrap();
    fs::write(edited.join("maps/custom_logo.png"), b"original-new-art").unwrap();

    let ok = Command::new(binary)
        .args(["mod", "diff"])
        .arg(&base)
        .arg(&edited)
        .arg(&out_mod)
        .args(["--id", "sparse-ok", "--name", "Sparse OK"])
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "strict mod diff should allow sparse cell edit + new asset: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let patch_text = fs::read_to_string(out_mod.join("patches/maps/010_Piata.json")).unwrap();
    assert!(patch_text.contains("\"cells\""), "{patch_text}");
    assert!(out_mod.join("assets/maps/custom_logo.png").is_file());

    // 2. Unkeyed base JSON array edit (`events`) is refused by default with exit code 2.
    let edited_unkeyed = r#"{
        "format_version": 2,
        "id": 16,
        "collision": {"plane": "bg", "plane_byte": 1, "width_cells": 2, "height_cells": 2, "rows": [[0, 0], [0, 8]]},
        "events": [0, 7]
    }"#;
    fs::write(edited.join("maps/010_Piata.json"), edited_unkeyed).unwrap();
    let bad_arr = Command::new(binary)
        .args(["mod", "diff"])
        .arg(&base)
        .arg(&edited)
        .arg(&out_mod)
        .args(["--id", "bad-arr", "--name", "Bad Arr"])
        .output()
        .unwrap();
    assert_eq!(bad_arr.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&bad_arr.stderr);
    assert!(stderr.contains("maps/010_Piata.json"), "{stderr}");
    assert!(stderr.contains("events"), "{stderr}");
    assert!(stderr.contains("--allow-derived"), "{stderr}");

    // 3. Modified base binary asset is refused by default with exit code 2.
    fs::write(edited.join("maps/010_Piata.json"), base_map).unwrap();
    fs::write(edited.join("maps/010_Piata.png"), b"modified-base-png").unwrap();
    let bad_bin = Command::new(binary)
        .args(["mod", "diff"])
        .arg(&base)
        .arg(&edited)
        .arg(&out_mod)
        .args(["--id", "bad-bin", "--name", "Bad Bin"])
        .output()
        .unwrap();
    assert_eq!(bad_bin.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&bad_bin.stderr);
    assert!(stderr.contains("maps/010_Piata.png"), "{stderr}");
    assert!(stderr.contains("--allow-derived"), "{stderr}");

    // 4. `--allow-derived` succeeds for both and prints a clear warning on stderr.
    fs::write(edited.join("maps/010_Piata.json"), edited_unkeyed).unwrap();
    let opt_in = Command::new(binary)
        .args(["mod", "diff"])
        .arg(&base)
        .arg(&edited)
        .arg(&out_mod)
        .args([
            "--id",
            "private-edit",
            "--name",
            "Private Edit",
            "--allow-derived",
        ])
        .output()
        .unwrap();
    assert!(
        opt_in.status.success(),
        "--allow-derived should succeed: {}",
        String::from_utf8_lossy(&opt_in.stderr)
    );
    let warn_stderr = String::from_utf8_lossy(&opt_in.stderr);
    assert!(warn_stderr.contains("--allow-derived"), "{warn_stderr}");
    assert!(warn_stderr.contains("maps/010_Piata.png"), "{warn_stderr}");
    assert!(warn_stderr.contains("events"), "{warn_stderr}");

    // 5. `mod init` rejects `--allow-derived` as an unknown flag.
    let bad_init = Command::new(binary)
        .args(["mod", "init"])
        .arg(scratch.join("init-bad"))
        .args(["--id", "init-bad", "--allow-derived"])
        .output()
        .unwrap();
    assert_eq!(bad_init.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&bad_init.stderr).contains("unknown flag"),
        "{}",
        String::from_utf8_lossy(&bad_init.stderr)
    );

    // 6. Real-pack `mod check` accepts hand-drawn replacement PNG under `assets/`
    //    with an advisory note (exit code 0) and warns on an unmodified base PNG copy.
    if let Some(pack) = real_pack() {
        let art_mod = scratch.join("original-art");
        let init = Command::new(binary)
            .args(["mod", "init"])
            .arg(&art_mod)
            .args(["--id", "original-art", "--name", "Original Art"])
            .output()
            .unwrap();
        assert!(init.status.success());
        fs::create_dir_all(art_mod.join("assets/maps")).unwrap();
        let orig_bytes = fs::read(pack.join("maps/010_Piata.png")).unwrap();
        let mut custom_bytes = orig_bytes.clone();
        custom_bytes.push(0x42);
        fs::write(art_mod.join("assets/maps/010_Piata.png"), &custom_bytes).unwrap();
        let check_art = Command::new(binary)
            .args(["mod", "check"])
            .arg(&pack)
            .arg(&art_mod)
            .output()
            .unwrap();
        assert!(
            check_art.status.success(),
            "hand-drawn replacement asset must pass mod check: {}",
            String::from_utf8_lossy(&check_art.stderr)
        );
        let art_stderr = String::from_utf8_lossy(&check_art.stderr);
        assert!(art_stderr.contains("note:"), "{art_stderr}");
        assert!(art_stderr.contains("maps/010_Piata.png"), "{art_stderr}");

        fs::write(art_mod.join("assets/maps/010_Piata.png"), &orig_bytes).unwrap();
        let check_dup = Command::new(binary)
            .args(["mod", "check"])
            .arg(&pack)
            .arg(&art_mod)
            .output()
            .unwrap();
        assert!(check_dup.status.success());
        let dup_stderr = String::from_utf8_lossy(&check_dup.stderr);
        assert!(dup_stderr.contains("warning:"), "{dup_stderr}");
        assert!(
            dup_stderr.contains("byte-for-byte identical"),
            "{dup_stderr}"
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}
