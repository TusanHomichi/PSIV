//! CLI tests for `psiv-pack mod <init|diff|check>` (#118).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn mod_init_diff_and_check_round_trip_and_refuse_invalid_mod() {
    let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime-pack absent; skipping psiv-pack mod CLI test");
        return;
    }

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
    assert!(stderr.contains("enemies.10.HP"), "{stderr}");

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
    let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime-pack absent; skipping MODDING.md CLI check");
        return;
    }

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
        3,
        "expected manifest + enemy + dialogue JSON examples in docs/MODDING.md"
    );

    let binary = env!("CARGO_BIN_EXE_psiv-pack");
    let scratch = std::env::temp_dir().join(format!("psiv-pack-mod-guide-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let mod_dir = scratch.join("hard-mode");
    fs::create_dir_all(mod_dir.join("patches/battle")).unwrap();
    fs::create_dir_all(mod_dir.join("patches/dialogue")).unwrap();
    fs::write(mod_dir.join("mod.json"), &json_blocks[0]).unwrap();
    fs::write(mod_dir.join("patches/battle/enemies.json"), &json_blocks[1]).unwrap();
    fs::write(mod_dir.join("patches/dialogue/trees.json"), &json_blocks[2]).unwrap();

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
        stdout.contains("ok: 1 mod(s), 2 patched file(s)"),
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
