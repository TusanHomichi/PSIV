//! Check the shared info-string cases against the actual pinned Rustdoc.
use serde::Deserialize;
use std::{fs, process::Command, time::SystemTime};

#[derive(Deserialize)]
struct Case {
    info: String,
    rust: bool,
}

#[test]
fn info_string_fixture_agrees_with_rustdoc() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("rustdoc_info.json")).unwrap();
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("psiv-float-rustdoc-{}-{nonce}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let source = root.join("examples").with_extension("rs");
    let mut text = String::new();
    for (index, case) in cases.iter().enumerate() {
        text.push_str(&format!(
            "/// ```{}\n/// let x = 1.5;\n/// ```\npub fn case_{index:03}() {{}}\n\n",
            case.info
        ));
    }
    fs::write(&source, text).unwrap();
    let output = Command::new(std::env::var_os("RUSTDOC").unwrap_or_else(|| "rustdoc".into()))
        .args(["--test", "--edition", "2024"])
        .arg(&source)
        .arg("--test-args=--list")
        .output()
        .unwrap();
    // This directory was freshly created for this test; no repository inputs.
    fs::remove_dir_all(&root).unwrap();
    assert!(
        output.status.success(),
        "Rustdoc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let listed = String::from_utf8(output.stdout).unwrap();
    for (index, case) in cases.iter().enumerate() {
        let name = format!("case_{index:03} (line ");
        assert_eq!(
            listed.contains(&name),
            case.rust,
            "info {:?}; Rustdoc listing:\n{listed}",
            case.info
        );
    }
}
