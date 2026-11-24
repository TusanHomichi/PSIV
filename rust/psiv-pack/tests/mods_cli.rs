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

    // 2. Patch enemy 10 HP and check via `psiv-pack mod check`
    fs::create_dir_all(init_dir.join("patches/battle")).unwrap();
    fs::write(
        init_dir.join("patches/battle/enemies.json"),
        r#"{"enemies":{"10":{"hp":77}}}"#,
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
        stdout.contains("ok: 1 mod(s), 1 patched file(s)"),
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

    let _ = fs::remove_dir_all(&scratch);
}
